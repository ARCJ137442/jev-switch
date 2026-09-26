use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use jev_core::{
    adapter::{Registry, UpstreamAdapter},
    router::{MatchMode, RouteEdge},
    upstream::{Capabilities, JevError, QuestionType},
};
use jev_protocol::{JevRequest, JevResponse};
use jev_switch_daemon::{build_app, AppState};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex, RwLock,
    },
};
use tower::ServiceExt;

struct Fake {
    calls: Arc<AtomicU32>,
    models: Arc<Mutex<Vec<String>>>,
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
    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.models.lock().unwrap().push(req.model);
        serde_json::from_str(r#"{"answers":{},"usage":{"input_tokens":2,"output_tokens":3}}"#)
            .map_err(|e| JevError::BadResponse {
                upstream_id: "fake".into(),
                message: e.to_string(),
            })
    }
}

struct FailingFake;
#[async_trait::async_trait]
impl UpstreamAdapter for FailingFake {
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
            retryable_status: &[500, 503],
        }
    }
    async fn evaluate(&self, _req: JevRequest) -> Result<JevResponse, JevError> {
        Err(JevError::Upstream {
            upstream_id: "fake".into(),
            status: 503,
            body: "private-upstream-error-sentinel".into(),
            retryable: true,
        })
    }
}

struct DelayedFake {
    calls: Arc<AtomicU32>,
    models: Arc<Mutex<Vec<String>>>,
}
#[async_trait::async_trait]
impl UpstreamAdapter for DelayedFake {
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
    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.models.lock().unwrap().push(req.model);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        serde_json::from_str(r#"{"answers":{},"usage":{"input_tokens":2,"output_tokens":3}}"#)
            .map_err(|e| JevError::BadResponse {
                upstream_id: "fake".into(),
                message: e.to_string(),
            })
    }
}

fn edge(left: &str) -> RouteEdge {
    RouteEdge {
        left: left.into(),
        r#match: MatchMode::Exact,
        right: "fake".into(),
        upstream_model: None,
        priority: 0,
        sticky: Default::default(),
        on_error: Default::default(),
    }
}
fn edge_model(left: &str, model: &str, priority: i32) -> RouteEdge {
    let mut route = edge(left);
    route.upstream_model = Some(model.into());
    route.priority = priority;
    route
}
fn edge_to(
    left: &str,
    right: &str,
    model: Option<&str>,
    priority: i32,
    sticky: jev_core::router::Sticky,
) -> RouteEdge {
    RouteEdge {
        left: left.into(),
        r#match: MatchMode::Exact,
        right: right.into(),
        upstream_model: model.map(str::to_owned),
        priority,
        sticky,
        on_error: Default::default(),
    }
}

async fn send(app: axum::Router, method: &str, uri: &str, body: &str) -> (u16, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn send_with_request_id(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: &str,
) -> (u16, Option<String>, Option<String>, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .header("origin", "http://localhost:5173")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-jev-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let exposed_headers = response
        .headers()
        .get("access-control-expose-headers")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        request_id,
        exposed_headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

async fn send_bearer(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: &str,
    token: &str,
) -> (u16, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn endpoint_routes_are_persisted_replaced_and_disabled_at_runtime() {
    let config_dir = std::env::temp_dir().join(format!("jev-endpoint-http-{}", std::process::id()));
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("providers.toml");
    std::fs::write(&config_path, "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\nmodels = [\"m1\", \"m2\"]\n").unwrap();
    let db = rusqlite::Connection::open_in_memory().unwrap();
    jev_switch_daemon::db::init_database(&db).unwrap();
    let db = Arc::new(Mutex::new(db));
    let calls = Arc::new(AtomicU32::new(0));
    let mut registry = Registry::new(vec![]);
    let models = Arc::new(Mutex::new(Vec::new()));
    registry.register(Box::new(Fake {
        calls: calls.clone(),
        models: models.clone(),
    }));
    let state = AppState {
        registry: Arc::new(registry),
        config_path,
        known_keys: Arc::new(RwLock::new(vec![])),
        auth: Arc::new(jev_switch_daemon::auth::AuthState::default()),
        listen: Arc::new(std::sync::OnceLock::new()),
        events: jev_switch_daemon::events::EventBus::new(16),
        service_endpoints: Arc::new(RwLock::new(HashMap::new())),
        db_conn: db.clone(),
    };
    let registry_handle = state.registry.clone();
    let app = build_app(state);
    let direct = r#"{"model":"m1","state":"s","questions":{"q":{"type":"noul","instructions":"direct-request-private-sentinel","criteria":{"true":"yes","false":"no"}}}}"#;
    let (status, response_request_id, exposed_headers, body) = send_with_request_id(
        app.clone(),
        "POST",
        "/v1/admin/providers/fake/invoke",
        direct,
    )
    .await;
    assert_eq!(
        status, 200,
        "direct provider invoke bypasses public route graph: {body}"
    );
    let direct_response: serde_json::Value = serde_json::from_str(&body).unwrap();
    let request_id = direct_response["request_id"]
        .as_str()
        .expect("direct call returns a durable request ID");
    assert!(request_id.starts_with("jev-"));
    assert_eq!(
        response_request_id.as_deref(),
        Some(request_id),
        "response header and JSON share the durable ID"
    );
    assert!(
        exposed_headers
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("x-jev-request-id"),
        "browser UI can read the correlation header across the dev origin"
    );
    assert_eq!(direct_response["route_trace"]["kind"], "direct_upstream");
    assert_eq!(direct_response["route_trace"]["provider_config_id"], "fake");
    assert_eq!(direct_response["route_trace"]["selected_model"], "m1");
    let stored: (String, String, i64, Option<String>, Option<String>) = db.lock().unwrap().query_row(
        "SELECT endpoint_id,upstream_model,http_status,request_id,route_trace_json FROM call_logs",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).unwrap();
    assert_eq!(stored.0, "direct:fake:m1");
    assert_eq!(stored.1, "m1");
    assert_eq!(stored.2, 200);
    assert_eq!(stored.3.as_deref(), Some(request_id));
    let stored_trace = stored.4.expect("direct call trace is persisted");
    assert!(stored_trace.contains("direct_upstream"));
    assert!(
        !stored_trace.contains("direct-request-private-sentinel"),
        "request content must not be persisted in traces"
    );
    let (status, body) = send(app.clone(), "GET", "/v1/admin/events?since=0&limit=10", "").await;
    assert_eq!(status, 200, "{body}");
    let events: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        events["events"].as_array().unwrap().len(),
        1,
        "direct calls share the durable activity feed"
    );
    let event_detail: serde_json::Value =
        serde_json::from_str(events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(event_detail["request_id"], request_id);
    assert_eq!(event_detail["endpoint_id"], "direct:fake:m1");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let wrong_model = direct.replace("m1", "unlisted-model");
    let (status, _) = send(
        app.clone(),
        "POST",
        "/v1/admin/providers/fake/invoke",
        &wrong_model,
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let create = serde_json::json!({"id":"public-a","strategy_config":{"type":"failover"},"routes":[edge_model("public-a", "m1", 0), edge_model("public-a", "m2", 1)] }).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create).await;
    assert_eq!(status, 200, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(created["routes_count"], 2);
    assert_eq!(created["health_summary"]["total"], 2);
    assert_eq!(created["health_summary"]["unknown"], 2);
    assert_eq!(created["health_summary"]["healthy"], 0);
    let (status, body) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        send(app.clone(), "GET", "/v1/admin/endpoints", ""),
    )
    .await
    .expect("endpoint list must not hang after creation");
    assert_eq!(status, 200, "{body}");
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(listed["endpoints"][0]["routes_count"], 2);
    let (health_status, _) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        send(app.clone(), "GET", "/health", ""),
    )
    .await
    .expect("health endpoint must remain responsive");
    assert_eq!(health_status, 200);
    let stored = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
        .unwrap()
        .unwrap()
        .routes;
    assert_eq!(
        stored,
        vec![
            edge_model("public-a", "m1", 0),
            edge_model("public-a", "m2", 1)
        ]
    );
    let candidates = registry_handle
        .router()
        .plan("public-a", &jev_core::router::RouteCtx::default())
        .unwrap();
    assert_eq!(
        candidates.len(),
        2,
        "same provider with two model ports must stay as two candidates"
    );
    assert_eq!(
        candidates
            .iter()
            .map(|item| item.candidate.upstream_model.as_str())
            .collect::<Vec<_>>(),
        ["m1", "m2"]
    );

    let request = r#"{"model":"public-a","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;
    let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let update = serde_json::json!({"id":"public-b","enabled":false,"routes":[edge("public-b")] })
        .to_string();
    let (status, body) = send(app.clone(), "PUT", "/v1/admin/endpoints/public-a", &update).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
            .unwrap()
            .unwrap()
            .routes,
        vec![edge("public-b")]
    );
    let (status, _) = send(app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(status, 404, "renamed old public id must stop resolving");
    let request_b = request.replace("public-a", "public-b");
    let (status, _) = send(app.clone(), "POST", "/v1/systemone", &request_b).await;
    assert_eq!(status, 404, "disabled public id must stop resolving");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "disabled/renamed calls must not reach provider"
    );
    let (status, body) = send(app.clone(), "GET", "/v1/admin/routes", "").await;
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains("public-b"),
        "disabled route stays editable in the saved graph: {body}"
    );

    let _ = std::fs::remove_dir_all(config_dir);
}

#[tokio::test]
async fn direct_provider_failures_are_durably_traced_without_persisting_error_bodies() {
    let config_dir =
        std::env::temp_dir().join(format!("jev-direct-failure-{}", std::process::id()));
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("providers.toml");
    std::fs::write(&config_path, "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\nmodels = [\"m1\"]\n").unwrap();
    let state = jev_switch_daemon::build_state(
        jev_switch_daemon::config::Config::load(&config_path).unwrap(),
        config_path.clone(),
    );
    state
        .registry
        .replace_upstreams(vec![Box::new(FailingFake)]);
    let db = state.db_conn.clone();
    let app = build_app(state);
    let request = r#"{"model":"m1","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;

    let (status, request_id, exposed_headers, body) = send_with_request_id(
        app.clone(),
        "POST",
        "/v1/admin/providers/fake/invoke",
        request,
    )
    .await;
    assert_eq!(status, 503, "{body}");
    assert!(
        request_id
            .as_deref()
            .is_some_and(|id| id.starts_with("jev-")),
        "failed calls remain correlatable from the response headers"
    );
    assert!(
        body.contains("private-upstream-error-sentinel"),
        "the caller still receives the upstream error"
    );
    let (status, body) = send(app, "GET", "/v1/admin/events?since=0&limit=10", "").await;
    assert_eq!(status, 200, "{body}");
    let events: serde_json::Value = serde_json::from_str(&body).unwrap();
    let detail: serde_json::Value =
        serde_json::from_str(events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(detail["success"], false);
    assert_eq!(detail["status"], 503);
    assert_eq!(detail["request_id"], request_id.unwrap());
    assert!(exposed_headers
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("x-jev-request-id"));
    assert_eq!(detail["provider"], "fake");
    let (error_message, route_trace): (Option<String>, Option<String>) = db
        .lock()
        .unwrap()
        .query_row(
            "SELECT error_message,route_trace_json FROM call_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(error_message.as_deref(), Some("HTTP 503"));
    assert!(
        !route_trace
            .unwrap()
            .contains("private-upstream-error-sentinel"),
        "raw upstream bodies are not persisted"
    );
    let _ = std::fs::remove_dir_all(config_dir);
}

#[tokio::test]
async fn endpoint_writes_reject_invalid_graphs_without_changing_runtime_or_storage() {
    let dir = std::env::temp_dir().join(format!("jev-endpoint-validation-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("providers.toml");
    std::fs::write(&path, "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\nmodels = [\"m1\"]\n").unwrap();
    let state = jev_switch_daemon::build_state(
        jev_switch_daemon::config::Config::load(&path).unwrap(),
        path.clone(),
    );
    let calls = Arc::new(AtomicU32::new(0));
    state.registry.replace_upstreams(vec![Box::new(Fake {
        calls: calls.clone(),
        models: Arc::new(Mutex::new(Vec::new())),
    })]);
    let db = state.db_conn.clone();
    let registry = state.registry.clone();
    let endpoints = state.service_endpoints.clone();
    let app = build_app(state);
    let create = serde_json::json!({"id":"stable","strategy_config":{"type":"failover"},"routes":[edge_model("stable","m1",0)]}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create).await;
    assert_eq!(status, 200, "{body}");
    // The cycle introduced later spans an existing intermediate node, so validating
    // only the submitted endpoint's outgoing edges would still miss it.
    let graph = vec![
        edge_model("stable", "m1", 0),
        edge_to("alias", "stable", None, 0, jev_core::router::Sticky::None),
    ];
    let (status, body) = send(
        app.clone(),
        "PUT",
        "/v1/admin/routes",
        &serde_json::json!({"routes":graph}).to_string(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let before = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
        .unwrap()
        .unwrap()
        .routes;
    let writes = [
        (
            "POST",
            "/v1/admin/endpoints",
            serde_json::json!({"id":"  ","strategy_config":{"type":"failover"}}),
        ),
        (
            "POST",
            "/v1/admin/endpoints",
            serde_json::json!({"id":"bad","strategy_config":{"type":"failover"},"routes":[edge_to("bad","ghost",None,0,jev_core::router::Sticky::None)]}),
        ),
        (
            "POST",
            "/v1/admin/endpoints",
            serde_json::json!({"id":"bad","strategy_config":{"type":"failover"},"routes":[edge_to("bad","bad",None,0,jev_core::router::Sticky::None)]}),
        ),
        (
            "PUT",
            "/v1/admin/endpoints/stable",
            serde_json::json!({"enabled":false,"routes":[edge_to("stable","alias",None,0,jev_core::router::Sticky::None)]}),
        ),
        (
            "PUT",
            "/v1/admin/endpoints/stable",
            serde_json::json!({"id":"renamed","enabled":false,"routes":[edge_to("renamed","ghost",None,0,jev_core::router::Sticky::None)]}),
        ),
        (
            "PUT",
            "/v1/admin/endpoints/stable",
            serde_json::json!({"id":"alias"}),
        ),
    ];
    for (method, uri, payload) in writes {
        let (status, body) = send(app.clone(), method, uri, &payload.to_string()).await;
        assert_eq!(
            status, 400,
            "invalid endpoint write {payload} must be rejected: {body}"
        );
        assert_eq!(
            jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
                .unwrap()
                .unwrap()
                .routes,
            before
        );
        assert_eq!(registry.router().edges(), before.as_slice());
        let saved = jev_switch_daemon::db::endpoints::load_all(&db.lock().unwrap()).unwrap();
        assert_eq!(
            saved.len(),
            1,
            "a rejected create/rename must not leave endpoint metadata"
        );
        assert_eq!(saved[0].id, "stable");
        assert!(saved[0].enabled);
        assert_eq!(endpoints.read().unwrap().len(), 1);
        assert!(endpoints.read().unwrap()["stable"].enabled);
        let request = r#"{"model":"stable","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;
        let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
        assert_eq!(
            status, 200,
            "the original entry must remain callable: {body}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 6);
    let (status, body) = send(
        app.clone(),
        "PUT",
        "/v1/admin/providers",
        r#"{"providers":[]}"#,
    )
    .await;
    assert_eq!(
        status, 400,
        "referenced providers must not be removed behind the graph: {body}"
    );
    let saved = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
        .unwrap()
        .unwrap();
    assert!(saved.providers.contains_key("fake"));
    assert_eq!(saved.routes, before);
    let (status, _) = send(app.clone(), "DELETE", "/v1/admin/endpoints/alias", "").await;
    assert_eq!(
        status, 404,
        "an intermediate alias is not a deletable public endpoint"
    );
    assert_eq!(registry.router().edges(), before.as_slice());

    let create = serde_json::json!({"id":"consumer","strategy_config":{"type":"failover"},"routes":[edge_to("consumer","alias",None,0,jev_core::router::Sticky::None),edge_model("consumer","m1",5)]}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create).await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = send(app.clone(), "DELETE", "/v1/admin/endpoints/stable", "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["routes_deleted"],
        3
    );
    let remaining = vec![edge_model("consumer", "m1", 5)];
    assert_eq!(
        registry.router().edges(),
        remaining.as_slice(),
        "the disconnected alias chain is removed but the other branch survives"
    );
    assert_eq!(
        jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
            .unwrap()
            .unwrap()
            .routes,
        remaining
    );
    let request = r#"{"model":"consumer","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;
    let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(
        status, 200,
        "surviving public branch must remain callable: {body}"
    );
    let (status, _) = send(
        app.clone(),
        "POST",
        "/v1/systemone",
        &request.replace("consumer", "stable"),
    )
    .await;
    assert_eq!(status, 404);
    drop(app);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn persisted_endpoint_route_is_restored_by_new_build_state() {
    let dir = std::env::temp_dir().join(format!("jev-endpoint-restart-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("providers.toml");
    std::fs::write(&config_path, "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\nmodels = [\"m1\", \"m2\"]\n").unwrap();
    let cfg = jev_switch_daemon::config::Config::load(&config_path).unwrap();
    let first_state = jev_switch_daemon::build_state(cfg, config_path.clone());
    let first_app = build_app(first_state);
    let create = serde_json::json!({"id":"persistent-model","strategy_config":{"type":"failover"},"routes":[edge_model("persistent-model", "m1", 20)] }).to_string();
    let (status, body) = send(first_app.clone(), "POST", "/v1/admin/endpoints", &create).await;
    assert_eq!(status, 200, "{body}");
    // Edit the published graph into a two-hop alias route and a direct fallback route.
    let graph = serde_json::json!({"routes":[
        edge_model("persistent-model", "m1", 10),
        edge_to("persistent-model", "fallback-main", None, 7, jev_core::router::Sticky::Session),
        edge_to("fallback-main", "fake", Some("m2"), 10, jev_core::router::Sticky::Session),
    ]})
    .to_string();
    let (status, body) = send(first_app.clone(), "PUT", "/v1/admin/routes", &graph).await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = send(first_app.clone(), "GET", "/v1/admin/routes", "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["routes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let (status, body) = send(
        first_app.clone(),
        "PUT",
        "/v1/admin/endpoints/persistent-model",
        r#"{"enabled":true}"#,
    )
    .await;
    assert_eq!(
        status, 200,
        "endpoint metadata PUT without routes must preserve the published graph: {body}"
    );
    drop(first_app);

    let cfg = jev_switch_daemon::config::Config::load(&config_path).unwrap();
    let mut restarted = jev_switch_daemon::build_state(cfg, config_path.clone());
    let calls = Arc::new(AtomicU32::new(0));
    let models = Arc::new(Mutex::new(Vec::new()));
    Arc::get_mut(&mut restarted.registry)
        .expect("registry not yet shared")
        .register(Box::new(Fake {
            calls: calls.clone(),
            models: models.clone(),
        }));
    let second_app = build_app(restarted);
    let (status, body) = send(second_app.clone(), "GET", "/v1/admin/routes", "").await;
    assert_eq!(status, 200, "{body}");
    let persisted_graph = serde_json::from_str::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        persisted_graph["routes"].as_array().unwrap().len(),
        3,
        "all public and alias edges survive restart"
    );
    let (status, body) = send(second_app.clone(), "GET", "/v1/models", "").await;
    assert_eq!(status, 200, "{body}");
    let public_models = serde_json::from_str::<serde_json::Value>(&body).unwrap();
    assert!(public_models["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|model| model["id"] == "persistent-model"));
    assert!(
        !public_models["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|model| model["id"] == "fallback-main"),
        "intermediate alias must not be published: {body}"
    );
    let request = r#"{"model":"persistent-model","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;
    let alias_request = request.replace("persistent-model", "fallback-main");
    let (status, _) = send(second_app.clone(), "POST", "/v1/systemone", &alias_request).await;
    assert_eq!(
        status, 404,
        "intermediate alias must not bypass the published endpoint boundary"
    );
    let (status, body) = send(second_app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(
        status, 200,
        "persisted route must be present after build_state: {body}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(models.lock().unwrap().as_slice(), ["m2"], "root priority 7 must order the alias branch before the direct 10 path even with no sticky key and permuted persisted edge order");
    drop(second_app);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn rejected_toml_import_keeps_database_runtime_and_restart_snapshot_unchanged() {
    let dir = std::env::temp_dir().join(format!("jev-import-atomic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("providers.toml");
    let valid_toml = "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\nmodels = [\"m1\"]\n";
    std::fs::write(&config_path, valid_toml).unwrap();

    let state = jev_switch_daemon::build_state(
        jev_switch_daemon::config::Config::load(&config_path).unwrap(),
        config_path.clone(),
    );
    let calls = Arc::new(AtomicU32::new(0));
    let models = Arc::new(Mutex::new(Vec::new()));
    state.registry.replace_upstreams(vec![Box::new(Fake {
        calls: calls.clone(),
        models: models.clone(),
    })]);
    let db = state.db_conn.clone();
    let registry = state.registry.clone();
    let app = build_app(state);
    let create = serde_json::json!({"id":"atomic-public","strategy_config":{"type":"failover"},"routes":[edge_model("atomic-public","m1",0)]}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &create).await;
    assert_eq!(status, 200, "{body}");
    let saved = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
        .unwrap()
        .unwrap();
    let original_routes = saved.routes.clone();
    let original_providers = saved.providers.clone();
    let request = r#"{"model":"atomic-public","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;

    let invalid_imports = [
        // Config parser rejects cycles before any database or runtime mutation.
        "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\n[[routes]]\nleft=\"atomic-public\"\nright=\"loop\"\n[[routes]]\nleft=\"loop\"\nright=\"atomic-public\"\n",
        // Unknown route references are rejected after parse but before commit.
        "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\n[[routes]]\nleft=\"atomic-public\"\nright=\"ghost\"\n",
        // An enabled unsupported adapter kind cannot be accepted while old adapters stay live.
        "[providers.fake]\nkind = \"unsupported\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\n[[routes]]\nleft=\"atomic-public\"\nright=\"fake\"\n",
    ];
    for content in invalid_imports {
        std::fs::write(&config_path, content).unwrap();
        let (status, body) = send(
            app.clone(),
            "POST",
            "/v1/admin/config/import-toml",
            r#"{"confirm":true}"#,
        )
        .await;
        assert_eq!(
            status, 400,
            "invalid import must be rejected before commit: {body}"
        );
        let current = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            current.routes, original_routes,
            "failed import changed saved routes"
        );
        assert_eq!(
            current.providers.keys().collect::<Vec<_>>(),
            original_providers.keys().collect::<Vec<_>>(),
            "failed import changed providers"
        );
        assert_eq!(
            registry.router().edges(),
            original_routes.as_slice(),
            "failed import changed current runtime graph"
        );
        let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
        assert_eq!(
            status, 200,
            "prior route must remain callable after rejected import: {body}"
        );
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "only the three post-rejection baseline calls reached the existing adapter"
    );

    // Force the baseline commit (which follows the file write) to fail; export must
    // restore the original TOML and leave the canonical runtime snapshot untouched.
    let original_file = std::fs::read(&config_path).unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_export_baseline BEFORE INSERT ON runtime_config BEGIN SELECT RAISE(ABORT, 'test baseline rejection'); END;").unwrap();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/config/export-toml", "{}").await;
    assert_eq!(
        status, 500,
        "forced baseline failure should be surfaced: {body}"
    );
    assert!(
        body.contains("original TOML restored"),
        "rollback result should be explicit: {body}"
    );
    assert_eq!(
        std::fs::read(&config_path).unwrap(),
        original_file,
        "failed export must restore prior TOML bytes"
    );
    let after_export_failure = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        after_export_failure.routes, original_routes,
        "failed export changed canonical routes"
    );
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_export_baseline;")
        .unwrap();

    // A fresh build restores exactly the persisted configuration even though the
    // rejected, unsupported TOML edit remains on disk for explicit user inspection.
    let restarted = jev_switch_daemon::build_state(
        jev_switch_daemon::config::Config::load(&config_path).unwrap(),
        config_path.clone(),
    );
    let restart_calls = Arc::new(AtomicU32::new(0));
    let restart_models = Arc::new(Mutex::new(Vec::new()));
    restarted.registry.replace_upstreams(vec![Box::new(Fake {
        calls: restart_calls.clone(),
        models: restart_models,
    })]);
    let restarted_app = build_app(restarted);
    let restored = jev_switch_daemon::db::load_runtime_snapshot(&db.lock().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(restored.routes, original_routes);
    assert_eq!(
        restored.providers.keys().collect::<Vec<_>>(),
        original_providers.keys().collect::<Vec<_>>()
    );
    let (status, body) = send(restarted_app, "POST", "/v1/systemone", request).await;
    assert_eq!(
        status, 200,
        "restarted runtime must use the unchanged SQLite snapshot: {body}"
    );
    assert_eq!(restart_calls.load(Ordering::SeqCst), 1);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn endpoint_and_global_strategy_settings_drive_real_scheduler_paths() {
    let dir = std::env::temp_dir().join(format!("jev-strategy-http-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("providers.toml");
    std::fs::write(
        &config_path,
        "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\n",
    )
    .unwrap();
    let db = rusqlite::Connection::open_in_memory().unwrap();
    jev_switch_daemon::db::init_database(&db).unwrap();
    jev_switch_daemon::db::save_runtime_snapshot(
        &db,
        &jev_switch_daemon::db::RuntimeConfigSnapshot {
            providers: jev_switch_daemon::config::Config::load(&config_path)
                .unwrap()
                .providers,
            routes: vec![],
            source_toml_fingerprint: "test".into(),
        },
    )
    .unwrap();
    let db = Arc::new(Mutex::new(db));
    let calls = Arc::new(AtomicU32::new(0));
    let models = Arc::new(Mutex::new(Vec::new()));
    let mut registry = Registry::new(vec![]);
    registry.register(Box::new(DelayedFake {
        calls: calls.clone(),
        models: models.clone(),
    }));
    let state = AppState {
        registry: Arc::new(registry),
        config_path,
        known_keys: Arc::new(RwLock::new(vec![])),
        auth: Arc::new(jev_switch_daemon::auth::AuthState::default()),
        listen: Arc::new(std::sync::OnceLock::new()),
        events: jev_switch_daemon::events::EventBus::new(32),
        service_endpoints: Arc::new(RwLock::new(HashMap::new())),
        db_conn: db,
    };
    let app = build_app(state);
    let request = r#"{"model":"strategy-public","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;
    let routes = vec![
        edge_model("strategy-public", "m1", 0),
        edge_model("strategy-public", "m2", 1),
    ];

    let race = serde_json::json!({"id":"strategy-public","strategy_config":{"type":"race","timeout_ms":1000},"routes":routes}).to_string();
    let (status, body) = send(app.clone(), "POST", "/v1/admin/endpoints", &race).await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(status, 200, "{body}");
    let trace: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(trace["route_trace"]["strategy"], "race", "{body}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "race must issue both eligible candidates concurrently"
    );

    models.lock().unwrap().clear();
    let update =
        serde_json::json!({"strategy_config":{"type":"load_balance","weight_mode":"equal"}})
            .to_string();
    let (status, body) = send(
        app.clone(),
        "PUT",
        "/v1/admin/endpoints/strategy-public",
        &update,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    for _ in 0..24 {
        let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
        assert_eq!(status, 200, "{body}");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["route_trace"]["strategy"], "load_balance");
        assert_eq!(parsed["route_trace"]["upstream_calls"], 1);
    }
    let selected: std::collections::HashSet<String> =
        models.lock().unwrap().iter().cloned().collect();
    assert!(
        selected.contains("m1") && selected.contains("m2"),
        "equal balance must select both model candidates across requests: {selected:?}"
    );

    let update =
        serde_json::json!({"strategy_config":{"type":"shadow","shadow_target":"fake"}}).to_string();
    let (status, body) = send(
        app.clone(),
        "PUT",
        "/v1/admin/endpoints/strategy-public",
        &update,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let before = calls.load(Ordering::SeqCst);
    let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(status, 200, "{body}");
    let trace: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(trace["route_trace"]["shadow"]["dispatched"], true);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) < before + 2 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("shadow target and primary must both execute");

    // Global JSON config retains parameters, and FollowGlobal consumes it at request time.
    let global = r#"{"default_strategy":"{\"type\":\"race\",\"timeout_ms\":250}"}"#;
    let (status, body) = send(
        app.clone(),
        "PUT",
        "/v1/admin/config/default_strategy",
        global,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let follow = serde_json::json!({"strategy_config":{"type":"follow_global"}}).to_string();
    let (status, body) = send(
        app.clone(),
        "PUT",
        "/v1/admin/endpoints/strategy-public",
        &follow,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = send(app.clone(), "POST", "/v1/systemone", request).await;
    assert_eq!(status, 200, "{body}");
    let trace: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(trace["route_trace"]["strategy"], "race");
    assert_eq!(trace["route_trace"]["upstream_calls"], 2);

    let bad_global = r#"{"default_strategy":"shadow"}"#;
    let (status, body) = send(app, "PUT", "/v1/admin/config/default_strategy", bad_global).await;
    assert_eq!(
        status, 400,
        "parameterless shadow must be rejected instead of silently becoming failover: {body}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn managed_tokens_enforce_roles_and_isolate_stats_and_events() {
    let config_dir = std::env::temp_dir().join(format!("jev-token-http-{}", std::process::id()));
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("providers.toml");
    std::fs::write(
        &config_path,
        "[providers.fake]\nkind = \"laya\"\nbase = \"http://127.0.0.1:9/v1/systemone\"\n",
    )
    .unwrap();
    let db = rusqlite::Connection::open_in_memory().unwrap();
    jev_switch_daemon::db::init_database(&db).unwrap();
    jev_switch_daemon::db::save_runtime_snapshot(
        &db,
        &jev_switch_daemon::db::RuntimeConfigSnapshot {
            providers: jev_switch_daemon::config::Config::load(&config_path)
                .unwrap()
                .providers,
            routes: vec![],
            source_toml_fingerprint: "test".into(),
        },
    )
    .unwrap();
    let admin = jev_switch_daemon::tokens::create(
        &db,
        "test admin",
        jev_switch_daemon::tokens::TokenRole::Admin,
    )
    .unwrap();
    let first = jev_switch_daemon::tokens::create(
        &db,
        "caller one",
        jev_switch_daemon::tokens::TokenRole::Readonly,
    )
    .unwrap();
    let second = jev_switch_daemon::tokens::create(
        &db,
        "caller two",
        jev_switch_daemon::tokens::TokenRole::Readonly,
    )
    .unwrap();
    let db = Arc::new(Mutex::new(db));
    let calls = Arc::new(AtomicU32::new(0));
    let mut registry = Registry::new(vec![]);
    let models = Arc::new(Mutex::new(Vec::new()));
    registry.register(Box::new(Fake {
        calls: calls.clone(),
        models,
    }));
    let state = AppState {
        registry: Arc::new(registry),
        config_path,
        known_keys: Arc::new(RwLock::new(vec![])),
        auth: Arc::new(jev_switch_daemon::auth::AuthState::default()),
        listen: Arc::new(std::sync::OnceLock::new()),
        events: jev_switch_daemon::events::EventBus::new(16),
        service_endpoints: Arc::new(RwLock::new(HashMap::new())),
        db_conn: db.clone(),
    };
    *state.auth.mode.write().unwrap() = jev_switch_daemon::config::RunMode::Cloud;
    let app = build_app(state);

    let (status, body) = send_bearer(app.clone(), "GET", "/v1/auth/me", "", &first.secret).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["role"],
        "readonly"
    );
    let (status, _) = send_bearer(app.clone(), "GET", "/v1/admin/tokens", "", &first.secret).await;
    assert_eq!(
        status, 401,
        "readonly token cannot read admin management APIs"
    );
    let (status, body) =
        send_bearer(app.clone(), "GET", "/v1/admin/tokens", "", &admin.secret).await;
    assert_eq!(status, 200, "admin call token may manage tokens: {body}");
    assert!(
        !body.contains(&admin.secret) && !body.contains(&first.secret),
        "list must never reveal secrets"
    );

    let create = serde_json::json!({"id":"token-public","strategy_config":{"type":"failover"},"routes":[edge_model("token-public","m1",0)]}).to_string();
    let (status, body) = send_bearer(
        app.clone(),
        "POST",
        "/v1/admin/endpoints",
        &create,
        &admin.secret,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let request = r#"{"model":"token-public","state":"s","questions":{"q":{"type":"noul","instructions":"test","criteria":{"true":"yes","false":"no"}}}}"#;
    let (status, body) =
        send_bearer(app.clone(), "POST", "/v1/systemone", request, &first.secret).await;
    assert_eq!(status, 200, "{body}");
    let response: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        response["latency_ms"].is_null(),
        "unknown provider-reported latency remains null"
    );
    assert!(
        response["cost_usd"].is_null(),
        "unknown cost remains null, never zero"
    );
    let gateway_latency = response["route_trace"]["gateway_latency_ms"]
        .as_u64()
        .expect("gateway request latency is separately measured");
    assert_eq!(
        response["route_trace"]["accounting"]["usage_cost_scope"],
        "selected_upstream_response_only"
    );
    let (status, body) = send_bearer(app.clone(), "GET", "/v1/stats/my", "", &first.secret).await;
    assert_eq!(status, 200, "{body}");
    let stats: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(stats["stats"]["total_requests"], 1);
    assert!(
        stats["stats"]["total_cost"].is_null(),
        "stats keep unknown aggregate cost null"
    );
    assert!(
        stats["stats"]["avg_latency_ms"].is_number(),
        "stats latency is measured gateway duration"
    );
    assert_eq!(stats["stats"]["token_id"], first.token.id);
    let (status, body) = send_bearer(
        app.clone(),
        "GET",
        "/v1/events/my?since=0&limit=10",
        "",
        &first.secret,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let events: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(events["events"].as_array().unwrap().len(), 1);
    assert_eq!(events["events"][0]["token_id"], first.token.id);
    let detail: serde_json::Value =
        serde_json::from_str(events["events"][0]["detail"].as_str().unwrap()).unwrap();
    assert_eq!(
        detail["latency_ms"].as_u64(),
        Some(gateway_latency),
        "response trace and persisted event use the same gateway clock"
    );
    assert_eq!(detail["upstream_calls"], 1);
    assert!(detail["cost_usd"].is_null());
    assert_eq!(
        detail["usage"]["input_tokens"], 2,
        "the reported usage survives into the caller event history"
    );
    let (status, body) = send_bearer(
        app.clone(),
        "GET",
        "/v1/events/my?since=0&limit=10",
        "",
        &second.secret,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["events"]
            .as_array()
            .unwrap()
            .is_empty(),
        "caller two cannot see caller one's events"
    );

    let (status, body) =
        send_bearer(app.clone(), "GET", "/v1/admin/stats", "", &admin.secret).await;
    assert_eq!(status, 200, "{body}");
    assert!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .any(|token| token["token_id"] == first.token.id)
    );
    let (status, _) = send_bearer(app.clone(), "GET", "/v1/admin/stats", "", &first.secret).await;
    assert_eq!(
        status, 401,
        "readonly token cannot read all-token statistics"
    );

    let (status, body) = send_bearer(
        app.clone(),
        "DELETE",
        &format!("/v1/admin/tokens/{}", first.token.id),
        "",
        &admin.secret,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, _) = send_bearer(app.clone(), "GET", "/v1/auth/me", "", &first.secret).await;
    assert_eq!(status, 401, "revoked token stops authenticating");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(app);
    let _ = std::fs::remove_dir_all(config_dir);
}
