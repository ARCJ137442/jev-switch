use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use jev_core::{
    adapter::UpstreamAdapter,
    upstream::{Capabilities, JevError, QuestionType},
};
use jev_protocol::{JevRequest, JevResponse};
use jev_switch_daemon::{
    build_app, build_state,
    config::Config,
    tokens::{self, TokenRole},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tokio::sync::Notify;
use tower::ServiceExt;

struct Fake {
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl UpstreamAdapter for Fake {
    fn id(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            question_types: &[
                QuestionType::Noul,
                QuestionType::Choice,
                QuestionType::Score,
                QuestionType::Boolean,
            ],
            has_confidence: true,
            has_usage: true,
            noul_via_boolean: false,
            retryable_status: &[500],
        }
    }
    async fn evaluate(&self, _req: JevRequest) -> Result<JevResponse, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        serde_json::from_str(r#"{"answers":{},"usage":{"input_tokens":2,"output_tokens":3},"cost_usd":0.25,"latency_ms":8}"#)
            .map_err(|error| JevError::BadResponse { upstream_id: "fake".into(), message: error.to_string() })
    }
}

struct RejectingFake {
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl UpstreamAdapter for RejectingFake {
    fn id(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            question_types: &[
                QuestionType::Noul,
                QuestionType::Choice,
                QuestionType::Score,
                QuestionType::Boolean,
            ],
            has_confidence: true,
            has_usage: false,
            noul_via_boolean: false,
            retryable_status: &[500],
        }
    }
    async fn evaluate(&self, _req: JevRequest) -> Result<JevResponse, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(JevError::Upstream {
            upstream_id: "fake".into(),
            status: 429,
            body: "private-upstream-diagnostic".into(),
            retryable: false,
        })
    }
}

struct GatedFake {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait::async_trait]
impl UpstreamAdapter for GatedFake {
    fn id(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            question_types: &[
                QuestionType::Noul,
                QuestionType::Choice,
                QuestionType::Score,
                QuestionType::Boolean,
            ],
            has_confidence: true,
            has_usage: true,
            noul_via_boolean: false,
            retryable_status: &[500],
        }
    }
    async fn evaluate(&self, _req: JevRequest) -> Result<JevResponse, JevError> {
        self.entered.notify_one();
        self.release.notified().await;
        serde_json::from_str(r#"{"answers":{},"usage":{"input_tokens":4,"output_tokens":1},"cost_usd":0.5,"latency_ms":11}"#)
            .map_err(|error| JevError::BadResponse { upstream_id: "fake".into(), message: error.to_string() })
    }
}

async fn send(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: &str,
    bearer: Option<&str>,
) -> (u16, String) {
    let (status, _, body) = send_with_headers(app, method, uri, body, bearer).await;
    (status, body)
}

async fn send_with_headers(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: &str,
    bearer: Option<&str>,
) -> (u16, Option<String>, String) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-jev-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        request_id,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn request_body(model: &str) -> String {
    serde_json::json!({
        "model": model,
        "state": "retention-test",
        "questions": {"q": {"type": "noul", "instructions": "test", "criteria": {"true": "yes", "false": "no"}}}
    }).to_string()
}

fn config_file(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("providers.toml");
    std::fs::write(&path, "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:1\"\nmodels = [\"m1\"]\napi_key = \"test-only\"\n").unwrap();
    path
}

async fn caller_snapshot(
    app: axum::Router,
    secret: &str,
) -> (serde_json::Value, serde_json::Value) {
    let (status, stats) = send(app.clone(), "GET", "/v1/stats/my", "", Some(secret)).await;
    assert_eq!(status, 200, "{stats}");
    let (status, events) = send(
        app,
        "GET",
        "/v1/events/my?since=0&limit=20",
        "",
        Some(secret),
    )
    .await;
    assert_eq!(status, 200, "{events}");
    (
        serde_json::from_str(&stats).unwrap(),
        serde_json::from_str(&events).unwrap(),
    )
}

#[tokio::test]
async fn endpoint_delete_keeps_token_history_with_foreign_keys_enabled_and_after_restart() {
    let dir = std::env::temp_dir().join(format!("jev-history-retention-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = config_file(&dir);
    let state = build_state(Config::load(&config_path).unwrap(), config_path.clone());
    let db = state.db_conn.clone();
    {
        let conn = db.lock().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        assert_eq!(
            conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    let token = tokens::create(&db.lock().unwrap(), "history caller", TokenRole::Readonly).unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    state.registry.replace_upstreams(vec![Box::new(Fake {
        calls: calls.clone(),
    })]);
    let app = build_app(state);

    let routes = serde_json::json!([{
        "left": "history-public", "match": "exact", "right": "fake", "upstream_model": "m1",
        "priority": 0, "sticky": "none", "on_error": "next"
    }]);
    let create = serde_json::json!({"id":"history-public","strategy_config":{"type":"failover"},"routes":routes}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create, None).await;
    assert_eq!(status, 200, "{body}");

    let (status, request_id_header, body) = send_with_headers(
        app.clone(),
        "POST",
        "/v1/systemone",
        &request_body("history-public"),
        Some(&token.secret),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let response: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (before_stats, before_events) = caller_snapshot(app.clone(), &token.secret).await;
    assert_eq!(before_stats["stats"]["total_requests"], 1);
    assert_eq!(before_stats["stats"]["total_cost"], 0.25);
    assert_eq!(before_events["events"].as_array().unwrap().len(), 1);
    let event_id = before_events["events"][0]["id"].as_i64().unwrap();
    let detail: serde_json::Value =
        serde_json::from_str(before_events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(detail["endpoint_id"], "history-public");
    assert_eq!(detail["success"], true);
    assert_eq!(detail["status"], 200);
    assert_eq!(detail["usage"]["input_tokens"], 2);
    assert_eq!(detail["cost_usd"], 0.25);
    assert_eq!(detail["request_id"], response["request_id"]);
    assert_eq!(
        request_id_header.as_deref(),
        response["request_id"].as_str()
    );
    assert_eq!(detail["route_trace"]["request_id"], detail["request_id"]);
    assert_eq!(detail["route_trace"]["selected_provider"], "fake");
    assert_eq!(response["route_trace"]["request_id"], detail["request_id"]);
    let trace_text = detail["route_trace"].to_string();
    assert!(
        !trace_text.contains("retention-test"),
        "route history must not store request bodies"
    );
    assert!(
        !trace_text.contains("instructions"),
        "route history must not store question content"
    );
    let stored: (Option<String>, Option<i64>, Option<String>) = db
        .lock()
        .unwrap()
        .query_row(
            "SELECT request_id,http_status,route_trace_json FROM call_logs WHERE id=?",
            [event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(stored.0.as_deref(), response["request_id"].as_str());
    assert_eq!(stored.1, Some(200));
    let stored_trace: serde_json::Value =
        serde_json::from_str(stored.2.as_deref().unwrap()).unwrap();
    assert_eq!(stored_trace, detail["route_trace"]);
    let (admin_status, admin_events_body) = send(
        app.clone(),
        "GET",
        "/v1/admin/events?since=0&limit=20",
        "",
        None,
    )
    .await;
    assert_eq!(admin_status, 200, "{admin_events_body}");
    let admin_events: serde_json::Value = serde_json::from_str(&admin_events_body).unwrap();
    assert_eq!(admin_events["events"][0]["id"], event_id);
    let admin_detail: serde_json::Value =
        serde_json::from_str(admin_events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(admin_detail["route_trace"]["selected_hops"][0], "fake");
    let (status, cursor_page) = send(
        app.clone(),
        "GET",
        &format!("/v1/admin/events?since={event_id}&limit=20"),
        "",
        None,
    )
    .await;
    assert_eq!(status, 200);
    let cursor_page: serde_json::Value = serde_json::from_str(&cursor_page).unwrap();
    assert_eq!(cursor_page["events"].as_array().unwrap().len(), 0);
    assert_eq!(cursor_page["next_since"], event_id);

    let original_log: serde_json::Value = {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id,timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,error_message,token_id,cost_usd,upstream_calls,usage_json FROM call_logs ORDER BY id").unwrap();
        let row = stmt
            .query_row([], |row| {
                Ok(serde_json::json!([
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<f64>>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, Option<String>>(12)?
                ]))
            })
            .unwrap();
        row
    };
    assert_eq!(original_log[0], event_id);

    let (status, body) = send(
        app.clone(),
        "DELETE",
        "/v1/admin/endpoints/history-public",
        "",
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, _) = send(
        app.clone(),
        "POST",
        "/v1/systemone",
        &request_body("history-public"),
        Some(&token.secret),
    )
    .await;
    assert_eq!(
        status, 404,
        "deleted public route must no longer be callable"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (after_stats, after_events) = caller_snapshot(app.clone(), &token.secret).await;
    assert_eq!(
        after_stats, before_stats,
        "endpoint deletion must not change caller statistics"
    );
    assert_eq!(
        after_events, before_events,
        "endpoint deletion must not change persistent event IDs/details/cursor"
    );
    {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id,timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,error_message,token_id,cost_usd,upstream_calls,usage_json FROM call_logs ORDER BY id").unwrap();
        let row = stmt
            .query_row([], |row| {
                Ok(serde_json::json!([
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<f64>>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, Option<String>>(12)?
                ]))
            })
            .unwrap();
        assert_eq!(
            row, original_log,
            "all persisted log columns and IDs must survive deletion"
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM service_endpoints WHERE id='history-public'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM routes WHERE left_node='history-public' OR right_node='history-public'", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }
    drop(app);
    drop(db);

    // A fresh build_state reopens the same sidecar DB with FK enforcement enabled
    // and must serve the exact same durable history/cursor without republishing.
    let restarted = build_state(Config::load(&config_path).unwrap(), config_path.clone());
    {
        let conn = restarted.db_conn.lock().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        assert_eq!(
            conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        let refs: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA foreign_key_list(call_logs)").unwrap();
            stmt.query_map([], |row| row.get(2))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            refs,
            vec!["call_tokens"],
            "history keeps only the caller-token FK, not a deletable endpoint FK"
        );
    }
    let app = build_app(restarted);
    let (after_restart_stats, after_restart_events) =
        caller_snapshot(app.clone(), &token.secret).await;
    assert_eq!(after_restart_stats, before_stats);
    assert_eq!(after_restart_events, before_events);
    let (admin_status, admin_events_body) = send(
        app.clone(),
        "GET",
        "/v1/admin/events?since=0&limit=20",
        "",
        None,
    )
    .await;
    assert_eq!(admin_status, 200, "{admin_events_body}");
    let admin_events: serde_json::Value = serde_json::from_str(&admin_events_body).unwrap();
    assert_eq!(admin_events["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        admin_events["events"][0]["id"], event_id,
        "admin history keeps the durable request cursor across restart"
    );
    let detail: serde_json::Value =
        serde_json::from_str(admin_events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(detail["request_id"], response["request_id"]);
    assert_eq!(detail["route_trace"]["selected_provider"], "fake");
    let (status, _) = send(
        app.clone(),
        "POST",
        "/v1/systemone",
        &request_body("history-public"),
        Some(&token.secret),
    )
    .await;
    assert_eq!(
        status, 404,
        "deleted endpoint must not be republished from stale TOML on restart"
    );
    drop(app);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn version_five_migration_preserves_full_rows_and_autoincrement_high_watermark() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../../../migrations/001_initial_schema.sql"))
        .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/002_runtime_config_snapshot.sql"
    ))
    .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/003_managed_call_tokens.sql"
    ))
    .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/004_call_usage_observability.sql"
    ))
    .unwrap();
    conn.execute_batch(include_str!("../../../migrations/005_data_migrations.sql"))
        .unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    jev_switch_daemon::db::endpoints::create(&conn, "history-v5", "follow_global", true).unwrap();
    conn.execute("INSERT INTO call_tokens (id,name,role,secret_hash,enabled,created_at) VALUES ('token-v5','test','readonly','hash-v5',1,1)", []).unwrap();
    conn.execute(
        "INSERT INTO call_logs (id,timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,error_message,token_id,cost_usd,upstream_calls,usage_json)
         VALUES (23,123,'history-v5','history-v5→fake','fake','m1',0,456,'HTTP 502','token-v5',0.75,2,'{\"input_tokens\":4,\"output_tokens\":1}')",
        [],
    ).unwrap();
    // Rows may have been pruned; sequence must still advance beyond 23.
    conn.execute(
        "UPDATE sqlite_sequence SET seq=900 WHERE name='call_logs'",
        [],
    )
    .unwrap();

    jev_switch_daemon::db::init_database(&conn).unwrap();

    let old: serde_json::Value = conn.query_row(
        "SELECT id,timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,error_message,token_id,cost_usd,upstream_calls,usage_json FROM call_logs",
        [], |row| Ok(serde_json::json!([
            row.get::<_,i64>(0)?, row.get::<_,i64>(1)?, row.get::<_,String>(2)?, row.get::<_,String>(3)?,
            row.get::<_,String>(4)?, row.get::<_,String>(5)?, row.get::<_,i64>(6)?, row.get::<_,i64>(7)?,
            row.get::<_,Option<String>>(8)?, row.get::<_,Option<String>>(9)?, row.get::<_,Option<f64>>(10)?,
            row.get::<_,Option<i64>>(11)?, row.get::<_,Option<String>>(12)?
        ])),
    ).unwrap();
    assert_eq!(
        old,
        serde_json::json!([
            23,
            123,
            "history-v5",
            "history-v5→fake",
            "fake",
            "m1",
            0,
            456,
            "HTTP 502",
            "token-v5",
            0.75,
            2,
            r#"{"input_tokens":4,"output_tokens":1}"#
        ])
    );
    assert_eq!(
        conn.query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='call_logs'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        900
    );
    conn.execute("INSERT INTO call_logs (timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,token_id) VALUES (124,'history-v5','history-v5','fake','m1',1,1,'token-v5')", []).unwrap();
    assert_eq!(
        conn.query_row("SELECT MAX(id) FROM call_logs", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        901
    );
    assert_eq!(
        conn.query_row("SELECT MAX(version) FROM migrations", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        conn.query_row("SELECT request_id FROM call_logs WHERE id=23", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .unwrap(),
        None
    );
    assert_eq!(
        conn.query_row(
            "SELECT route_trace_json FROM call_logs WHERE id=23",
            [],
            |row| row.get::<_, Option<String>>(0)
        )
        .unwrap(),
        None
    );
}

#[test]
fn version_five_migration_preserves_sequence_when_call_log_table_is_empty() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../../../migrations/001_initial_schema.sql"))
        .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/002_runtime_config_snapshot.sql"
    ))
    .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/003_managed_call_tokens.sql"
    ))
    .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/004_call_usage_observability.sql"
    ))
    .unwrap();
    conn.execute_batch(include_str!("../../../migrations/005_data_migrations.sql"))
        .unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    jev_switch_daemon::db::endpoints::create(&conn, "empty-history", "follow_global", true)
        .unwrap();
    conn.execute("INSERT INTO call_tokens (id,name,role,secret_hash,enabled,created_at) VALUES ('token-empty','test','readonly','hash-empty',1,1)", []).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM call_logs", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    conn.execute(
        "INSERT INTO sqlite_sequence (name,seq) VALUES ('call_logs',900)",
        [],
    )
    .unwrap();

    jev_switch_daemon::db::init_database(&conn).unwrap();

    conn.execute("INSERT INTO call_logs (timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,token_id) VALUES (124,'empty-history','empty-history','fake','m1',1,1,'token-empty')", []).unwrap();
    assert_eq!(
        conn.query_row("SELECT id FROM call_logs", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        901
    );
}

#[tokio::test]
async fn accepted_inflight_call_is_persisted_if_endpoint_is_deleted_before_completion() {
    let dir = std::env::temp_dir().join(format!("jev-inflight-retention-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = config_file(&dir);
    let state = build_state(Config::load(&config_path).unwrap(), config_path.clone());
    {
        let conn = state.db_conn.lock().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    }
    let token = tokens::create(
        &state.db_conn.lock().unwrap(),
        "inflight caller",
        TokenRole::Readonly,
    )
    .unwrap();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    state.registry.replace_upstreams(vec![Box::new(GatedFake {
        entered: entered.clone(),
        release: release.clone(),
    })]);
    let db = state.db_conn.clone();
    let app = build_app(state);
    let routes = serde_json::json!([{"left":"inflight-public","match":"exact","right":"fake","upstream_model":"m1","priority":0,"sticky":"none","on_error":"next"}]);
    let create = serde_json::json!({"id":"inflight-public","strategy_config":{"type":"failover"},"routes":routes}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create, None).await;
    assert_eq!(status, 200, "{body}");

    let request_app = app.clone();
    let request_secret = token.secret.clone();
    let request = tokio::spawn(async move {
        send(
            request_app,
            "POST",
            "/v1/systemone",
            &request_body("inflight-public"),
            Some(&request_secret),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), entered.notified())
        .await
        .expect("fake upstream must be entered before deletion");

    let (status, body) = send(
        app.clone(),
        "DELETE",
        "/v1/admin/endpoints/inflight-public",
        "",
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    release.notify_one();
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(3), request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        status, 200,
        "request already admitted before deletion must finish: {body}"
    );
    let (stats, events) = caller_snapshot(app.clone(), &token.secret).await;
    assert_eq!(
        stats["stats"]["total_requests"], 1,
        "admitted request remains attributable to caller"
    );
    assert_eq!(stats["stats"]["total_cost"], 0.5);
    assert_eq!(events["events"].as_array().unwrap().len(), 1);
    let detail: serde_json::Value =
        serde_json::from_str(events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(
        detail["endpoint_id"], "inflight-public",
        "history keeps the id accepted at request entry"
    );
    let (status, _) = send(
        app.clone(),
        "POST",
        "/v1/systemone",
        &request_body("inflight-public"),
        Some(&token.secret),
    )
    .await;
    assert_eq!(
        status, 404,
        "a later request must not use the deleted entry"
    );
    assert_eq!(
        db.lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM call_logs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );

    drop(app);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn failed_call_history_persists_sanitized_route_trace_and_retry_decision() {
    let dir = std::env::temp_dir().join(format!("jev-failed-trace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = config_file(&dir);
    let state = build_state(Config::load(&config_path).unwrap(), config_path.clone());
    let db = state.db_conn.clone();
    let token = tokens::create(
        &db.lock().unwrap(),
        "failed trace caller",
        TokenRole::Readonly,
    )
    .unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    state
        .registry
        .replace_upstreams(vec![Box::new(RejectingFake {
            calls: calls.clone(),
        })]);
    let app = build_app(state);

    let routes = serde_json::json!([{
        "left":"failed-trace-public", "match":"exact", "right":"fake", "upstream_model":"m1",
        "priority":0, "sticky":"none", "on_error":"fail"
    }]);
    let create = serde_json::json!({"id":"failed-trace-public","strategy_config":{"type":"failover"},"routes":routes}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create, None).await;
    assert_eq!(status, 200, "{body}");

    let (status, request_id, _) = send_with_headers(
        app.clone(),
        "POST",
        "/v1/systemone",
        &request_body("failed-trace-public"),
        Some(&token.secret),
    )
    .await;
    assert_eq!(status, 429);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let request_id = request_id.expect("failed upstream request still gets a traceable request ID");

    let (_, events) = caller_snapshot(app, &token.secret).await;
    assert_eq!(events["events"].as_array().unwrap().len(), 1);
    let event_id = events["events"][0]["id"].as_i64().unwrap();
    let detail: serde_json::Value =
        serde_json::from_str(events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(detail["request_id"], request_id);
    assert_eq!(detail["success"], false);
    assert_eq!(detail["status"], 429);
    assert_eq!(detail["provider"], "fake");
    assert_eq!(detail["upstream_model"], "m1");
    assert_eq!(detail["upstream_calls"], 1);
    assert_eq!(detail["route_trace"]["strategy"], "failover");
    assert_eq!(detail["route_trace"]["outcome"], "failed");
    assert_eq!(
        detail["route_trace"]["attempts"][0]["retry_decision"],
        "stop_by_candidate_policy"
    );
    assert_eq!(detail["route_trace"]["attempts"][0]["upstream_status"], 429);
    assert!(!detail["route_trace"]
        .to_string()
        .contains("private-upstream-diagnostic"));
    assert!(!detail["route_trace"].to_string().contains("retention-test"));

    let stored: (Option<String>, Option<i64>, Option<i64>, Option<String>) = db.lock().unwrap().query_row(
        "SELECT request_id,http_status,upstream_calls,route_trace_json FROM call_logs WHERE id=?",
        [event_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap();
    assert_eq!(stored.0.as_deref(), Some(request_id.as_str()));
    assert_eq!(stored.1, Some(429));
    assert_eq!(stored.2, Some(1));
    let persisted_trace: serde_json::Value =
        serde_json::from_str(stored.3.as_deref().unwrap()).unwrap();
    assert_eq!(persisted_trace, detail["route_trace"]);

    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}
