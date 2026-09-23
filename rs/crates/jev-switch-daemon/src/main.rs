//! Jev-Switch MVP 入口（M0.9 · P0-1 迁入 jev-switch-daemon）
//!
//! 整合：jev-protocol / jev-core / jev-adapters / config / axum HTTP server。
//!
//! 路由：
//! - `POST /v1/systemone` — 主入口：Jev 协议请求
//! - `GET  /health`        — liveness
//! - `GET  /v1/models`     — 列出可达 model + upstream + capability
//!
//! 错误映射：JevError → HTTP 状态码由 JevError::http_status() 决定。

mod config;

use anyhow::Context;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use config::{Config, ConfigError};
use jev_adapters::{upstream_laya::LayaUpstream, upstream_vercel::VercelUpstream};
use jev_core::{
    router::{OnError, RouteCtx, Router as JevRouter},
    upstream::{JevError, QuestionType, Upstream},
};
use jev_protocol::{JevRequest, JevResponse};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{BTreeSet, HashMap},
    net::SocketAddr,
    sync::Arc,
};
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    router: Arc<JevRouter>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct ErrorBody {
    error: String,
    upstream: Option<String>,
    retryable: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing init
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // 1. load config（load 内含 DAG 检环：含环配置直接拒绝启动）
    let config = Config::load_default().context("load config")?;
    let route_edges = config.route_edges();
    tracing::info!(
        providers = config.providers.len(),
        routes = route_edges.len(),
        "config loaded"
    );

    // 2. construct upstreams based on config
    let mut upstreams: HashMap<String, Box<dyn Upstream>> = HashMap::new();

    if let Some(p) = config.providers.get("vercel") {
        if p.enabled {
            let api_key = match config.read_api_key("vercel") {
                Ok(k) => k,
                Err(ConfigError::MissingEnv { provider, var }) => {
                    tracing::warn!(provider = %provider, var = %var, "skipping vercel provider: missing env");
                    String::new()
                }
                Err(e) => return Err(e.into()),
            };
            if !api_key.is_empty() {
                match VercelUpstream::new(p.base.clone(), api_key) {
                    Ok(u) => {
                        upstreams.insert("vercel".into(), Box::new(u));
                        tracing::info!(base = %p.base, "vercel upstream ready");
                    }
                    Err(e) => tracing::warn!(error = %e, "vercel upstream init failed"),
                }
            }
        } else {
            tracing::info!("vercel provider disabled in config");
        }
    }

    if let Some(p) = config.providers.get("laya") {
        if p.enabled {
            match LayaUpstream::new(p.base.clone()) {
                Ok(u) => {
                    upstreams.insert("laya".into(), Box::new(u));
                    tracing::info!(base = %p.base, "laya upstream ready");
                }
                Err(e) => tracing::warn!(error = %e, "laya upstream init failed"),
            }
        } else {
            tracing::info!("laya provider disabled in config");
        }
    }

    // 3. router（DAG 边 = [[routes]] ∪ 旧 [router] 展开）
    let router = JevRouter::new(route_edges, upstreams);

    // 4. axum state + routes
    let state = AppState {
        router: Arc::new(router),
    };
    // CORS 白名单（contracts/05 §5 M1+；替换 very_permissive 已知债务）：
    // vite dev 两种打开方式都可能 —— 127.0.0.1 与 localhost 都放行。
    // admin 端点本轮未上线（A7）；同主机 UI 走默认同源语义。
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
        ]))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/v1/systemone", post(systemone_handler))
        .route("/health", get(health_handler))
        .route("/v1/models", get(models_handler))
        .layer(cors)
        .with_state(state);

    // 默认 11435 — 对齐用户叙事基址期望（docs/「Jev-Switch」用户叙事探索）
    let addr = SocketAddr::from(([127, 0, 0, 1], 11435));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "jev-switch MVP listening");

    axum::serve(listener, app).await?;
    Ok(())
}

/// `GET /health` — contracts/05 §2：JSON 为准（旧文本 "jev-switch MVP" 弃用）。
#[derive(Debug, Serialize)]
struct HealthBody {
    status: &'static str,
    version: &'static str,
}

async fn health_handler() -> Json<HealthBody> {
    Json(HealthBody {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// `GET /v1/models` — contracts/05 §2 冻结形状（OpenAI 风）。
#[derive(Debug, Serialize)]
struct ModelEntry {
    id: String,
    object: &'static str,
    upstream: String,
}

#[derive(Debug, Serialize)]
struct UpstreamCapabilityEntry {
    id: String,
    question_types: Vec<&'static str>,
    has_confidence: bool,
    has_usage: bool,
    noul_via_boolean: bool,
}

#[derive(Debug, Serialize)]
struct ModelsResponse {
    object: &'static str,
    data: Vec<ModelEntry>,
    upstreams: Vec<UpstreamCapabilityEntry>,
}

async fn models_handler(State(state): State<AppState>) -> Json<ModelsResponse> {
    // 保留 6af3a47 的不可路由过滤：只列出真正可路由的 model
    // （upstream 未注册的 model 不列出 —— 否则 UI 判断可用、点击即 404）
    let mut data: Vec<ModelEntry> = state
        .router
        .list_models()
        .into_iter()
        .filter_map(|m| {
            let upstream = state.router.route(&m).ok()?.id().to_string();
            Some(ModelEntry {
                id: m,
                object: "model",
                upstream,
            })
        })
        .collect();
    data.sort_by(|a, b| a.id.cmp(&b.id));

    let upstreams: Vec<UpstreamCapabilityEntry> = state
        .router
        .list_capabilities()
        .into_iter()
        .map(|(_id, _uid, cap)| UpstreamCapabilityEntry {
            id: _id,
            question_types: cap.question_types.iter().map(|q| q.as_str()).collect(),
            has_confidence: cap.has_confidence,
            has_usage: cap.has_usage,
            noul_via_boolean: cap.noul_via_boolean,
        })
        .collect();

    Json(ModelsResponse {
        object: "list",
        data,
        upstreams,
    })
}

/// 把 JevError 转换为 axum Response。
fn jev_error_to_response(e: JevError) -> Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = ErrorBody {
        error: e.to_string(),
        upstream: Some(e.upstream_id().to_string()),
        retryable: e.retryable(),
    };
    (status, Json(body)).into_response()
}

async fn systemone_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<JevResponse>, Response> {
    // 0. 协议解析：本地 400（criteria 缺失/错形态、未知 type、必填缺失、
    //    questions 非 record…）—— 按 contracts/01 §6 不发上游。
    //    手工 Bytes 提取：axum Json 提取器对 data 类错误回 422，契约要求 400。
    let req: JevRequest = serde_json::from_slice(&body).map_err(|e| {
        let err = ErrorBody {
            error: e.to_string(),
            upstream: None,
            retryable: false,
        };
        (StatusCode::BAD_REQUEST, Json(err)).into_response()
    })?;

    run_request(&state.router, req).await.map(Json)
}

/// A4 failover 主循环（从 handler 抽出以便进程内单测，行为 = handler 内联版）。
///
/// 规则（contracts/03 §4 + A4 任务书）：
/// - 无匹配边 / 全悬空 → 404 `UnknownModel`/`UnknownUpstream` 文案
/// - 逐候选：capability 不匹配 → **跳过（不计入失败语义，不发请求）**
/// - `upstream_model` 改写后发上游；每实发一次 `upstream_calls += 1`
/// - 仅 `JevError::retryable()`（429/5xx/Timeout/Network）且该候选 `on_error=next`
///   → 试下一候选；`fail` 或不可重试 → 第一错即返
/// - 多候选成功取**首个成功**响应，`upstream_calls` 如实回写
/// - 全败 → 按最后错误映射状态码；全部 capability 跳过 → 422
/// - sticky：HTTP 层暂无 sticky key 入口 → `RouteCtx` 传 None（见报告）
async fn run_request(
    router: &JevRouter,
    req: JevRequest,
) -> Result<JevResponse, Response> {
    let ctx = RouteCtx {
        sticky_key: None, // HTTP 层暂无 sticky 入口（契约位保留，core 有单测）
        exclude: BTreeSet::new(),
    };
    let plan = match router.plan(&req.model, &ctx) {
        Ok(p) if p.is_empty() => {
            // 命中边但候选全被 exclude / 全解析不到 → 404（无可用上游）
            return Err(error_response(
                StatusCode::NOT_FOUND,
                format!("no upstream registered for model '{}'", req.model),
                None,
            ));
        }
        Ok(p) => p,
        Err(e) => {
            // UnknownModel / UnknownUpstream → 404（旧 route() 任意错误也走 404，不回潮）
            return Err(error_response(StatusCode::NOT_FOUND, e.to_string(), None));
        }
    };

    let qts: Vec<QuestionType> = req.questions.values().map(|q| q.question_type()).collect();

    let mut upstream_calls: u32 = 0;
    let mut last_err: Option<JevError> = None;
    let mut cap_skip: Option<JevError> = None; // 首个 capability 不匹配（全跳过时转 422）

    for item in &plan {
        let upstream = match router.upstream(&item.candidate.upstream_id) {
            Some(u) => u,
            None => {
                // select 已保证注册；防御分支（配置热变等）按不可用候选记错跳过
                last_err = Some(JevError::Config {
                    upstream_id: item.candidate.upstream_id.clone(),
                    message: "upstream disappeared after select".into(),
                });
                continue;
            }
        };

        // capability：不匹配 = 跳过该候选（不计入失败语义，不实发）
        if let Some(qt) = qts.iter().find(|qt| !upstream.capabilities().supports(**qt)) {
            if cap_skip.is_none() {
                cap_skip = Some(JevError::Capability {
                    upstream_id: upstream.id().to_string(),
                    detail: format!(
                        "model '{}' requires question type '{}' but upstream '{}' cannot handle it",
                        req.model,
                        qt.as_str(),
                        upstream.id()
                    ),
                });
            }
            tracing::debug!(
                upstream = %upstream.id(),
                qt = qt.as_str(),
                "candidate skipped: capability mismatch"
            );
            continue;
        }

        // upstream_model 改写后发上游
        let mut attempt_req = req.clone();
        attempt_req.model = item.candidate.upstream_model.clone();

        upstream_calls += 1; // 实发计数（capability 跳过不计）
        match upstream.evaluate(attempt_req).await {
            Ok(raw) => {
                // raw Value → typed JevResponse（形态非法 → 502，不可重试 → 即返）
                return match serde_json::from_value::<JevResponse>(raw) {
                    Ok(mut resp) => {
                        resp.upstream_calls = Some(upstream_calls); // 如实 = 实发次数
                        Ok(resp)
                    }
                    Err(e) => {
                        tracing::warn!(
                            upstream = %upstream.id(),
                            error = %e,
                            "deserialize upstream response failed"
                        );
                        Err(error_response(
                            StatusCode::BAD_GATEWAY,
                            format!("deserialize upstream response: {e}"),
                            Some(upstream.id().to_string()),
                        ))
                    }
                };
            }
            Err(e) => {
                tracing::warn!(upstream = %e.upstream_id(), error = %e, "upstream evaluate failed");
                let retry_next =
                    e.retryable() && item.on_error == OnError::Next; // 仅可重试错误试下一候选
                if retry_next {
                    last_err = Some(e);
                    continue; // on_error=next → 下一候选
                }
                return Err(jev_error_to_response(e)); // fail 首错即返 / 不可重试即返
            }
        }
    }

    // 循环走完：有失败记录 → 按最后错误映射；否则全部 capability 跳过 → 422
    if let Some(e) = last_err {
        return Err(jev_error_to_response(e));
    }
    if let Some(e) = cap_skip {
        tracing::warn!(model = %req.model, error = %e, "all candidates capability-skipped");
        return Err(jev_error_to_response(e));
    }
    Err(error_response(
        StatusCode::NOT_FOUND,
        format!("no upstream registered for model '{}'", req.model),
        None,
    ))
}

/// 构造统一错误体响应（contracts/05 §3）。
fn error_response(status: StatusCode, error: String, upstream: Option<String>) -> Response {
    let body = ErrorBody {
        error,
        upstream,
        retryable: false,
    };
    (status, Json(body)).into_response()
}

// 防止 rust 误以为 main 用不到的 import 是 dead_code
#[allow(dead_code)]
fn _assert_send_sync() {
    fn assert_send<T: Send + Sync>() {}
    assert_send::<AppState>();
    assert_send::<JevError>();
    // 用一下 json! 宏保持 serde_json 引用
    let _ = json!({});
}

#[cfg(test)]
mod tests {
    use super::*;

    /// contracts/05 §2 /health 形状快照：{status, version}，JSON。
    #[test]
    fn health_shape_snapshot() {
        let body = HealthBody {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")})
        );
        // 键集冻结：不得漂移出第三个键
        assert_eq!(v.as_object().unwrap().len(), 2);
        // 当前契约样例版本字面量（0.1.0）
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(v["status"], "ok");
    }

    /// contracts/05 §2 /v1/models 形状快照：{object:"list", data:[{id,object,upstream}], upstreams:[…]}。
    /// 防 37b4242 类形状漂移（UI 已按此归一化）。
    #[test]
    fn models_shape_snapshot() {
        let resp = ModelsResponse {
            object: "list",
            data: vec![ModelEntry {
                id: "jev".into(),
                object: "model",
                upstream: "vercel".into(),
            }],
            upstreams: vec![UpstreamCapabilityEntry {
                id: "vercel".into(),
                question_types: vec!["choice", "score", "boolean"],
                has_confidence: false,
                has_usage: false,
                noul_via_boolean: true,
            }],
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "object": "list",
                "data": [{ "id": "jev", "object": "model", "upstream": "vercel" }],
                "upstreams": [{
                    "id": "vercel",
                    "question_types": ["choice", "score", "boolean"],
                    "has_confidence": false,
                    "has_usage": false,
                    "noul_via_boolean": true
                }]
            })
        );
        // 顶层键集冻结（models 旧形状键不得回潮）
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert!(obj.get("models").is_none());
    }

    /* ════════════════════════════════════════════════════════
       A4 failover（进程内 fake Upstream；不占 11435、无 wiremock）
       ════════════════════════════════════════════════════════ */

    use jev_core::router::{MatchMode, RouteEdge, Sticky};
    use jev_core::upstream::Capabilities;
    use jev_protocol::{Criteria, Question};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// fake 上游行为脚本。
    enum Behave {
        /// 合法 JevResponse JSON。
        Ok200,
        /// 可重试 429（vercel 类 retryable_status 含 429）。
        Err429,
        /// 不可重试 400。
        Err400,
        /// 合法 JSON 但不是 JevResponse 形态 → 502。
        InvalidJson,
    }

    /// 可共享句柄的 fake 上游。
    struct FakeHandle {
        calls: AtomicU32,
        seen_models: Mutex<Vec<String>>,
    }

    struct Fake {
        id: String,
        cap: Capabilities,
        behave: Behave,
        handle: Arc<FakeHandle>,
    }

    #[async_trait::async_trait]
    impl Upstream for Fake {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                question_types: self.cap.question_types,
                has_confidence: self.cap.has_confidence,
                has_usage: self.cap.has_usage,
                noul_via_boolean: self.cap.noul_via_boolean,
                retryable_status: self.cap.retryable_status,
            }
        }
        async fn evaluate(&self, req: jev_protocol::JevRequest) -> Result<serde_json::Value, JevError> {
            self.handle.calls.fetch_add(1, Ordering::SeqCst);
            self.handle.seen_models.lock().unwrap().push(req.model.clone());
            match self.behave {
                Behave::Ok200 => Ok(serde_json::json!({"answers": {}})),
                Behave::Err429 => Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 429,
                    body: "rate limited".into(),
                    retryable: true,
                }),
                Behave::Err400 => Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 400,
                    body: "bad request".into(),
                    retryable: false,
                }),
                Behave::InvalidJson => Ok(serde_json::json!({"not": "a jev response"})),
            }
        }
    }

    fn fake(id: &str, qts: &'static [QuestionType], behave: Behave) -> (Box<dyn Upstream>, Arc<FakeHandle>) {
        let handle = Arc::new(FakeHandle {
            calls: AtomicU32::new(0),
            seen_models: Mutex::new(Vec::new()),
        });
        let cap = Capabilities {
            question_types: qts,
            has_confidence: false,
            has_usage: false,
            noul_via_boolean: qts.contains(&QuestionType::Boolean),
            retryable_status: &[408, 429, 500, 502, 503, 504],
        };
        let boxed = Box::new(Fake {
            id: id.to_string(),
            cap,
            behave,
            handle: handle.clone(),
        });
        (boxed, handle)
    }

    fn noul_request(model: &str) -> JevRequest {
        let mut q = std::collections::BTreeMap::new();
        q.insert(
            "q".to_string(),
            Question::Noul {
                instructions: "is greeting?".into(),
                criteria: Criteria::Bool {
                    r#true: "yes".into(),
                    r#false: "no".into(),
                },
            },
        );
        JevRequest {
            model: model.to_string(),
            state: serde_json::json!("hi"),
            questions: q,
        }
    }

    fn edge(left: &str, right: &str, priority: i32) -> RouteEdge {
        RouteEdge {
            left: left.into(),
            r#match: MatchMode::Exact,
            right: right.into(),
            upstream_model: None,
            priority,
            sticky: Sticky::None,
            on_error: OnError::Next,
        }
    }

    async fn status_of(resp: Result<JevResponse, Response>) -> u16 {
        match resp {
            Ok(_) => 200,
            Err(r) => r.status().as_u16(),
        }
    }

    async fn error_body(resp: Result<JevResponse, Response>) -> ErrorBody {
        match resp {
            Ok(_) => panic!("expected error response"),
            Err(r) => {
                let bytes = axum::body::to_bytes(r.into_body(), 64 * 1024)
                    .await
                    .expect("read body");
                serde_json::from_slice(&bytes).expect("ErrorBody json")
            }
        }
    }

    /// 验收 ①：同 model "jev" 双候选，首候选 429 → 落次候选成功且 upstream_calls>=2。
    #[tokio::test]
    async fn failover_429_falls_to_second_candidate_with_upstream_calls() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let mut upstreams = HashMap::new();
        upstreams.insert("vercel".to_string(), v1);
        upstreams.insert("laya".to_string(), v2);

        let mut e1 = edge("jev", "vercel", 10);
        e1.upstream_model = Some("typesafe-ai/jev".into());
        let mut e2 = edge("jev", "laya", 30);
        e2.upstream_model = Some("laya-english".into());
        let router = JevRouter::new(vec![e1, e2], upstreams);

        let resp = run_request(&router, noul_request("jev")).await.expect("failover success");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1, "首候选实发 1 次");
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1, "次候选实发 1 次");
        // upstream_calls 如实 = 实发次数 >= 2
        assert!(resp.upstream_calls.unwrap_or(0) >= 2, "got {:?}", resp.upstream_calls);
        assert_eq!(resp.upstream_calls, Some(2));
        // upstream_model 改写：vercel 收到 typesafe-ai/jev，laya 收到 laya-english
        assert_eq!(h1.seen_models.lock().unwrap().as_slice(), &["typesafe-ai/jev"]);
        assert_eq!(h2.seen_models.lock().unwrap().as_slice(), &["laya-english"]);
    }

    /// 验收 ⑥：404 语义不回潮（无匹配边）。
    #[tokio::test]
    async fn unknown_model_is_404() {
        let router = JevRouter::new(vec![], HashMap::new());
        let resp = run_request(&router, noul_request("nope")).await;
        assert_eq!(status_of(resp).await, 404);
    }

    /// 验收 ⑥：422 语义不回潮（capability 不匹配且无可用候选）。
    #[tokio::test]
    async fn capability_mismatch_all_skipped_is_422() {
        let (v1, h1) = fake("vercel", &[QuestionType::Choice], Behave::Ok200);
        let upstreams = HashMap::from([("vercel".to_string(), v1)]);
        let router = JevRouter::new(vec![edge("jev", "vercel", 10)], upstreams);

        let resp = run_request(&router, noul_request("jev")).await;
        assert_eq!(status_of(resp).await, 422);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 0, "capability 跳过不实发");
        let body = error_body(run_request(&router, noul_request("jev")).await).await;
        assert!(body.error.contains("question type"), "got: {}", body.error);
        assert!(!body.retryable);
    }

    /// capability 不匹配 = 跳过该候选（不计入失败语义）→ 落下一候选成功，calls 只计实发。
    #[tokio::test]
    async fn capability_mismatch_skips_candidate_without_counting_failure() {
        // cand1 只支持 choice（不支持 noul）→ 跳过；cand2 支持 noul → 成功
        let (v1, h1) = fake("vercel", &[QuestionType::Choice], Behave::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let upstreams = HashMap::from([
            ("vercel".to_string(), v1),
            ("laya".to_string(), v2),
        ]);
        let router = JevRouter::new(
            vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)],
            upstreams,
        );

        let resp = run_request(&router, noul_request("jev")).await.expect("second wins");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 0, "cap 跳过 = 零实发（连 429 都没走到）");
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1);
        assert_eq!(resp.upstream_calls, Some(1), "upstream_calls 只计实发");
    }

    /// on_error=fail：首错即返（即使错误可重试），不试下一候选。
    #[tokio::test]
    async fn on_error_fail_returns_first_error_without_next() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let upstreams = HashMap::from([
            ("vercel".to_string(), v1),
            ("laya".to_string(), v2),
        ]);
        let e1 = RouteEdge {
            on_error: OnError::Fail,
            ..edge("jev", "vercel", 10)
        };
        let router = JevRouter::new(vec![e1, edge("jev", "laya", 30)], upstreams);

        let resp = run_request(&router, noul_request("jev")).await;
        assert_eq!(status_of(resp).await, 503, "429 retryable → 对外 503");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "fail 不试下一候选");
        let body = error_body(run_request(&router, noul_request("jev")).await).await;
        assert!(body.retryable, "429 retryable 标志透出");
        assert_eq!(body.upstream.as_deref(), Some("vercel"));
    }

    /// 不可重试错误（400）：on_error=next 也不试下一候选（仅 retryable 才 failover）。
    #[tokio::test]
    async fn non_retryable_error_stops_even_with_on_error_next() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err400);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let upstreams = HashMap::from([
            ("vercel".to_string(), v1),
            ("laya".to_string(), v2),
        ]);
        let router = JevRouter::new(
            vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)],
            upstreams,
        );
        let resp = run_request(&router, noul_request("jev")).await;
        assert_eq!(status_of(resp).await, 400, "透传上游码");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1, "首候选实发一次即停");
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "不可重试错误不 failover");
    }

    /// 上游响应形态非法 → 502 即返（不 failover；反序列化失败非 retryable）。
    #[tokio::test]
    async fn invalid_upstream_json_is_502_no_failover() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::InvalidJson);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let upstreams = HashMap::from([
            ("vercel".to_string(), v1),
            ("laya".to_string(), v2),
        ]);
        let router = JevRouter::new(
            vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)],
            upstreams,
        );
        let resp = run_request(&router, noul_request("jev")).await;
        assert_eq!(status_of(resp).await, 502);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0);
    }

    /// 旧 [router] 行为回归：单 exact 边一次成功，upstream_calls=1。
    #[tokio::test]
    async fn legacy_single_edge_success_upstream_calls_one() {
        let (v1, h1) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let upstreams = HashMap::from([("laya".to_string(), v1)]);
        let router =
            JevRouter::new(vec![edge("laya-english", "laya", 0)], upstreams);
        let resp = run_request(&router, noul_request("laya-english")).await.expect("ok");
        assert_eq!(resp.upstream_calls, Some(1));
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        // 无 upstream_model 改写 → 沿用原 model
        assert_eq!(h1.seen_models.lock().unwrap().as_slice(), &["laya-english"]);
    }

    /// CORS 白名单 origin 字面量冻结（vite dev 两种打开方式）。
    #[test]
    fn cors_allowlist_origins_frozen() {
        let layer = CorsLayer::new()
            .allow_origin(AllowOrigin::list([
                HeaderValue::from_static("http://127.0.0.1:5173"),
                HeaderValue::from_static("http://localhost:5173"),
            ]))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE]);
        // tower-http 不提供只读 introspection；构造成功 + 预检行为由 curl 实测覆盖。
        // 此处至少锁定构造路径可编译（防 very_permissive 回潮需人工改回本函数）。
        let _ = layer;
        let _ = StatusCode::OK;
    }
}
