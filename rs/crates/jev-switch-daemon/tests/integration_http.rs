//! A8 · axum oneshot 集成基座（契约级 HTTP 回归护栏）。
//!
//! 进程内 `build_app(state)` + `tower::ServiceExt::oneshot` —— **不绑端口**
//! （不占 11435、与 B 线并行安全；上游走 wiremock 随机端口 / 进程内 fake）。
//!
//! 覆盖（任务书 8 条）：
//! 1. `GET /health` 形状（单元快照已有 —— 此处 HTTP 级引一）
//! 2. 未知 model → 404 错误体（`upstream:null`）
//! 3. 题型 capability 不匹配 → 422（**回归护栏**：零实发不回潮）
//! 4. 协议非法（缺 criteria）→ 400
//! 5. retryable 上游错误 → 503 + `retryable:true`（wiremock）
//! 6. failover：双候选首败次成 → 200 + `upstream_calls==2`（wiremock）
//! 7. `GET /v1/models` 不可路由过滤（**回归护栏**：6af3a47）
//! 8. `GET /v1/admin/providers` 无明文（HTTP 级复核 —— 与 A7 单测不同层）
//!
//! #43 注：基座全部 local 态（`state_with` → `AuthState::default()`）——
//! 上述 8 条同时兼任「local 态零鉴权回归」的 HTTP 级护栏；cloud 态鉴权
//! 401/200/login 全套在 `src/auth.rs` 模块测试。

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use jev_adapters::upstream_laya::LayaUpstream;
use jev_adapters::upstream_vercel::VercelUpstream;
use jev_core::adapter::{Registry, RetryPolicy, UpstreamAdapter};
use jev_core::router::{MatchMode, RouteEdge};
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse};
use jev_switch_daemon::{build_app, build_state, config::Config, AppState};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use tower::ServiceExt;

/* ══════════════════════════════════════════════════════════════════
   夹具
   ══════════════════════════════════════════════════════════════════ */

/// 测试专用假 key（真实密钥永不进测试/提交）。
const FAKE_KEY: &str = "sk-test1234abcd";

/// 每测独立临时配置文件（不碰 `JEV_SWITCH_CONFIG` 环境变量 —— 进程级会打架）。
fn temp_config(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("jev-itg-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("providers.toml");
    std::fs::write(&path, content).unwrap();
    path
}

/// 直接构造 state（自定义 Registry —— fake/wiremock 上游；admin 路径用 temp config）。
/// `auth` 用 `Default`（mode=local —— 集成基座跑 local 态回归；cloud 态鉴权有
/// `auth.rs` 模块测试专覆盖）。`listen` 留空 OnceLock（oneshot 路径：
/// mode 翻转 rebind.skipped="no listener"、`PUT /listen` → 503）。
fn state_with(registry: Registry, config_path: std::path::PathBuf) -> AppState {
    AppState {
        registry: Arc::new(registry),
        config_path,
        known_keys: Arc::new(RwLock::new(Vec::new())),
        auth: Arc::new(jev_switch_daemon::auth::AuthState::default()),
        listen: Arc::new(std::sync::OnceLock::new()),
        events: jev_switch_daemon::events::EventBus::new(200),
    }
}

/// 发一次请求，回 `(status, body 字符串)`；app 被 oneshot 消耗（每用例一个）。
async fn send(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> (u16, String) {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    let req = match body {
        Some(b) => builder.body(Body::from(b)).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(req).await.expect("oneshot");
    let status = resp.status().as_u16();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

fn edge(left: &str, right: &str, priority: i32) -> RouteEdge {
    RouteEdge {
        left: left.into(),
        r#match: MatchMode::Exact,
        right: right.into(),
        upstream_model: None,
        priority,
        sticky: Default::default(),
        on_error: Default::default(),
    }
}

/// 合法 noul 请求（JSON 字面 —— 与 wire 上一致）。
fn noul_body(model: &str) -> String {
    serde_json::json!({
        "model": model,
        "state": "hi",
        "questions": {
            "q": {
                "type": "noul",
                "instructions": "is greeting?",
                "criteria": {"true": "yes", "false": "no"}
            }
        }
    })
    .to_string()
}

/// 进程内计数 fake 上游（capability 护栏用 —— 记录是否被实发）。
struct CountingFake {
    id: String,
    cap: Capabilities,
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl UpstreamAdapter for CountingFake {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> Capabilities {
        self.cap.clone()
    }
    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _ = req;
        serde_json::from_str(r#"{"answers":{}}"#).map_err(|e| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: e.to_string(),
        })
    }
}

/// 仅支持 choice 的 fake（对 noul 请求 = capability 不匹配）。
fn choice_only_fake(id: &str, calls: Arc<AtomicU32>) -> Box<dyn UpstreamAdapter> {
    Box::new(CountingFake {
        id: id.to_string(),
        cap: Capabilities {
            question_types: &[QuestionType::Choice],
            has_confidence: false,
            has_usage: false,
            noul_via_boolean: false,
            retryable_status: &[408, 429, 500, 502, 503, 504],
        },
        calls,
    })
}

/* ══════════════════════════════════════════════════════════════════
   1 · GET /health 形状（HTTP 级）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn health_json_shape_over_http() {
    let path = temp_config("health", "[providers.laya]\nkind=\"laya\"\nbase=\"http://127.0.0.1:1/x\"\nenabled=false\n");
    let cfg = Config::load(&path).unwrap();
    let app = build_app(build_state(cfg, path.clone()));

    let (status, body) = send(app, "GET", "/health", None).await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["version"].is_string());
    assert_eq!(v.as_object().unwrap().len(), 2, "键集冻结: {body}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   2 · 未知 model → 404 错误体（upstream:null）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn unknown_model_is_404_with_null_upstream() {
    let path = temp_config("404", "# empty\n");
    let reg = Registry::new(vec![]);
    let app = build_app(state_with(reg, path.clone()));

    let (status, body) = send(app, "POST", "/v1/systemone", Some(noul_body("ghost-model"))).await;
    assert_eq!(status, 404, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["upstream"], serde_json::Value::Null, "404 必须 upstream:null: {body}");
    assert_eq!(v["retryable"], false);
    assert!(v["error"].as_str().unwrap().contains("ghost-model"), "{body}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   3 · capability 不匹配 → 422（回归护栏 · 零实发）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn capability_mismatch_is_422_without_real_send() {
    let path = temp_config("422", "# empty\n");
    let calls = Arc::new(AtomicU32::new(0));
    let mut reg = Registry::new(vec![edge("jev", "choice-only", 10)]);
    reg.register(choice_only_fake("choice-only", calls.clone()));
    let app = build_app(state_with(reg, path.clone()));

    let (status, body) = send(app, "POST", "/v1/systemone", Some(noul_body("jev"))).await;
    assert_eq!(status, 422, "题型不匹配必须 422（6af3a47 不可回潮）: {body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        v["error"].as_str().unwrap().contains("question type"),
        "{body}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "capability 跳过 = 零实发");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   4 · 协议非法（缺 criteria）→ 400
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn missing_criteria_is_400() {
    let path = temp_config("400", "# empty\n");
    let app = build_app(state_with(Registry::new(vec![]), path.clone()));

    // 缺 criteria 字段（contracts/01：反序列化即校验 → 本地 400，不发上游）
    let body_json = serde_json::json!({
        "model": "jev",
        "state": "hi",
        "questions": {
            "q": { "type": "noul", "instructions": "x" }
        }
    })
    .to_string();
    let (status, body) = send(app, "POST", "/v1/systemone", Some(body_json)).await;
    assert_eq!(status, 400, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["retryable"], false);
    assert_eq!(v["upstream"], serde_json::Value::Null, "{body}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   5 · retryable 上游错误 → 503 + retryable:true（wiremock）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn retryable_upstream_error_maps_to_503() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let path = temp_config("503", "# empty\n");
    let mut reg = Registry::with_retry(
        vec![edge("jev", "vercel", 10)],
        RetryPolicy::no_retry(), // 隔离重试时延：只测错误映射
    );
    reg.register(Box::new(
        VercelUpstream::new(format!("{}/evaluate", server.uri()), "itg-test-key".into())
            .expect("vercel upstream"),
    ));
    let app = build_app(state_with(reg, path.clone()));

    let (status, body) = send(app, "POST", "/v1/systemone", Some(noul_body("jev"))).await;
    assert_eq!(status, 503, "429 retryable → 503: {body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["retryable"], true, "{body}");
    assert_eq!(v["upstream"], "vercel", "{body}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   6 · failover：双候选首败次成 → 200 + upstream_calls==2（wiremock）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn failover_first_fail_second_ok_counts_two_calls() {
    // 首候选：vercel → 恒 429（retryable）
    let mock_vercel = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&mock_vercel)
        .await;
    // 次候选：laya → 200 标准 JevResponse
    let mock_laya = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"answers": {}})),
        )
        .mount(&mock_laya)
        .await;

    let path = temp_config("failover", "# empty\n");
    let mut reg = Registry::with_retry(
        vec![
            {
                let mut e = edge("jev", "vercel", 10);
                e.upstream_model = Some("typesafe-ai/jev".into());
                e
            },
            {
                let mut e = edge("jev", "laya", 30);
                e.upstream_model = Some("laya-english".into());
                e
            },
        ],
        RetryPolicy::no_retry(), // 隔离：首败即跨候选
    );
    reg.register(Box::new(
        VercelUpstream::new(format!("{}/evaluate", mock_vercel.uri()), "itg-test-key".into())
            .expect("vercel"),
    ));
    reg.register(Box::new(
        LayaUpstream::new(format!("{}/v1/systemone", mock_laya.uri())).expect("laya"),
    ));
    let app = build_app(state_with(reg, path.clone()));

    let (status, body) = send(app, "POST", "/v1/systemone", Some(noul_body("jev"))).await;
    assert_eq!(status, 200, "failover 后应 200: {body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["upstream_calls"], 2, "如实计数实发 2 次: {body}");

    // wiremock 侧核对：vercel 恰 1 发（no_retry）、laya 恰 1 发
    assert_eq!(
        mock_vercel.received_requests().await.unwrap().len(),
        1,
        "首候选实发 1 次"
    );
    assert_eq!(
        mock_laya.received_requests().await.unwrap().len(),
        1,
        "次候选实发 1 次"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   7 · GET /v1/models 不可路由过滤（回归护栏 · 6af3a47）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn models_filters_unroutable_entries() {
    let path = temp_config("models", "# empty\n");
    // 只注册 laya；ghost-model 的 right=vercel 悬空 → 不可路由 → 不得列出
    let mut reg = Registry::new(vec![
        edge("ghost-model", "vercel", 10),
        edge("keep-model", "laya", 10),
    ]);
    reg.register(Box::new(
        LayaUpstream::new("http://127.0.0.1:1/v1/systemone".into()).expect("laya"),
    ));
    let app = build_app(state_with(reg, path.clone()));

    let (status, body) = send(app, "GET", "/v1/models", None).await;
    assert_eq!(status, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["object"], "list", "{body}");

    let ids: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["keep-model"], "不可路由 ghost-model 必须被过滤（6af3a47）: {body}");
    assert!(v["upstreams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u["id"] == "laya"), "能力列表来自注册制 trait: {body}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/* ══════════════════════════════════════════════════════════════════
   8 · GET /v1/admin/providers 无明文（HTTP 级复核）
   ══════════════════════════════════════════════════════════════════ */

#[tokio::test]
async fn admin_providers_over_http_has_no_plaintext() {
    let cfg_toml = format!(
        r#"
[providers.vercel]
kind = "vercel"
base = "https://example.invalid/v4/eval"
api_key = "{FAKE_KEY}"
enabled = true

[providers.laya]
kind = "laya"
base = "http://127.0.0.1:18765/v1/systemone"
enabled = true
"#
    );
    let path = temp_config("admin-mask", &cfg_toml);
    let cfg = Config::load(&path).unwrap();
    let app = build_app(build_state(cfg, path.clone()));

    let (status, body) = send(app, "GET", "/v1/admin/providers", None).await;
    assert_eq!(status, 200, "{body}");

    // HTTP 级：全响应字符串不含配置明文；含掩码；无裸 api_key 字段
    assert!(!body.contains(FAKE_KEY), "HTTP 响应泄露明文: {body}");
    assert!(body.contains("sk-****abcd"), "{body}");
    assert!(!body.contains(r#""api_key":"#), "{body}");
    assert!(body.contains(r#""api_key_masked""#), "{body}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
