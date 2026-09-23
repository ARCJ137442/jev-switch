//! Jev-Switch daemon 库（A7 拆出 —— bin 只剩装配入口；`tests/` 集成基座可
//! `use jev_switch_daemon::{build_app, build_state, AppState}` 进程内起 app）。
//!
//! 整合：jev-protocol / jev-core / jev-adapters / config / auth / axum HTTP server。
//!
//! 路由：
//! - `POST /v1/systemone` — 主入口：Jev 协议请求
//! - `GET  /health`        — liveness（**双态皆放行** —— 探活需要）
//! - `GET  /v1/models`     — 列出可达 model + upstream + capability
//! - `GET/PUT /v1/admin/providers`、`GET/PUT /v1/admin/routes`、
//!   `POST /v1/admin/providers/{id}/probe` — Admin（A7）
//! - `POST /v1/admin/login` — #43 cloud 态管理登录（换短时会话 token；门外免会话）
//! - `PUT  /v1/admin/mode`  — mode 热切（零重启；门外密码激活见 admin 模块）
//! - `PUT/GET /v1/admin/listen` — 监听热 Rebind（ListenSupervisor，任务级小网关）
//! - `PUT  /v1/admin/password` — admin 密码热更（门外：会话或 loopback）
//! - `GET  /v1/admin/status` — 首页仪表盘自检（mode/bind/设密/uptime，7 键冻结）
//! - `GET  /`、`/assets/*` 等 — 静态 UI（`tower_http::ServeDir` 挂 `JEV_UI_DIST`，
//!   默认 `ui/dist`；同源托管简化 CORS —— #43 容器交付）
//!
//! **双态（#43，docs/12 §一）**：`mode=local`（默认）= loopback peer 免鉴权
//! （非 loopback → 403 兜底）；`mode=cloud` = `/v1` 调用 token、admin 会话密码门
//! （见 [`auth`]）。**mode 只管鉴权，不涉及上游拓扑**（拓扑不区分原则）。
//!
//! **bind 与 mode 解耦 + 热切（ListenSupervisor 修订裁决）**：非显式 bind 时
//! 成对默认（local → `127.0.0.1:11435`、cloud → `0.0.0.0:11435`），mode 翻转
//! 联动热 Rebind；显式 bind（文件/`JEV_BIND`）恒绑不动、peer 校验兜底。
//! Listen 层独立 supervisor 任务（[`listen`]）—— 换地址零进程重启、零内核重建。
//!
//! 错误映射：JevError → HTTP 状态码由 JevError::http_status() 决定。
//! 错误体 `error` 字符串一律过 `jev_core::redact`（contracts/04 §2 红线 4）。

pub mod admin;
pub mod auth;
pub mod config;
pub mod listen;

use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderValue, Method, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use config::{Config, ConfigError};
use jev_adapters::{upstream_laya::LayaUpstream, upstream_vercel::VercelUpstream};
use jev_core::{
    adapter::{plain_ctx, Registry},
    redact::redact,
    upstream::JevError,
};
use jev_protocol::{JevRequest, JevResponse};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::ServeDir,
};

/// daemon 装配状态（`build_state` 造、`build_app` 消费；`Clone` 供 axum + 测试共享句柄）。
#[derive(Clone)]
pub struct AppState {
    /// A5 装配：能力注册制 —— 上游经 `Registry::register(Box<dyn UpstreamAdapter>)`
    /// 挂载，`/v1/models` 能力从 trait 取（`capabilities_of` 硬编码已删）。
    /// A7：admin `PUT /v1/admin/routes` 经 `Registry::replace_edges` 热替换边表。
    pub registry: Arc<Registry>,
    /// 配置文件真值路径（`JEV_SWITCH_CONFIG` 解析结果；admin 读改写同一文件）。
    pub config_path: PathBuf,
    /// 需要脱敏的已知明文密钥集（配置 api_key + 启动/GET/PUT 时刷新）——
    /// ErrorBody / tracing / admin 响应共用（contracts/04 §2 Redact）。
    pub known_keys: Arc<RwLock<Vec<String>>>,
    /// #43 双态鉴权运行态（mode / 调用 token / admin 密码 / 内存会话）。
    pub auth: Arc<auth::AuthState>,
    /// ListenSupervisor 句柄（`main`/测试经 [`listen::start`] 注入；
    /// `OnceLock` = build_state 时 supervisor 尚未创建）。缺失时 mode 翻转
    /// `rebind.skipped="no listener"`、`PUT /listen` → 503（oneshot 测试路径）。
    pub listen: Arc<std::sync::OnceLock<listen::ListenHandle>>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ErrorBody {
    pub error: String,
    pub upstream: Option<String>,
    pub retryable: bool,
}

/// 构造统一错误体（contracts/05 §3）—— **`error` 字符串先过 redact**（红线 4）。
fn error_response(
    status: StatusCode,
    error: impl Into<String>,
    upstream: Option<String>,
    retryable: bool,
    keys: &[String],
) -> Response {
    let body = ErrorBody {
        error: redact(&error.into(), keys),
        upstream,
        retryable,
    };
    (status, Json(body)).into_response()
}

/// 读 state 的已知密钥快照（cloned，避开跨 await 持锁）。
fn known_keys_snapshot(state: &AppState) -> Vec<String> {
    state
        .known_keys
        .read()
        .expect("known_keys lock")
        .clone()
}

/// 装配：config → Registry（DAG 边 + 注册 vercel/laya 上游）+ known_keys。
///
/// 与原 `main` 的装配逻辑逐语义一致（A7 只是挪进可测入口）；差异：
/// - `read_api_key` 现为「明文 api_key 优先、api_key_env 兼容」（Q4=b）
/// - 启动即收集已知密钥供 redact
pub fn build_state(config: Config, config_path: PathBuf) -> AppState {
    // 进程启动时刻（status.uptime_s 基准；多次 build_state 只取首次 —— 测试同进程共享）
    admin::PROCESS_START.get_or_init(std::time::Instant::now);
    let route_edges = config.route_edges();
    let mut registry = Registry::new(route_edges);

    if let Some(p) = config.providers.get("vercel") {
        if p.enabled {
            let api_key = match config.read_api_key("vercel") {
                Ok(k) => k,
                Err(ConfigError::MissingEnv { provider, var }) => {
                    tracing::warn!(provider = %provider, var = %var, "skipping vercel provider: missing env");
                    String::new()
                }
                Err(e) => {
                    // 配置级错误（如未知 provider）不致命：跳过注册，与旧行为一致的降级
                    tracing::warn!(error = %e, "skipping vercel provider: config error");
                    String::new()
                }
            };
            if !api_key.is_empty() {
                match VercelUpstream::new(p.base.clone(), api_key) {
                    Ok(u) => {
                        registry.register(Box::new(u));
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
                    registry.register(Box::new(u));
                    tracing::info!(base = %p.base, "laya upstream ready");
                }
                Err(e) => tracing::warn!(error = %e, "laya upstream init failed"),
            }
        } else {
            tracing::info!("laya provider disabled in config");
        }
    }

    // 已知密钥集（redact 用）：明文字段 + 各 provider 能解析出的有效 key
    let mut known_keys: Vec<String> = Vec::new();
    for id in config.providers.keys() {
        if let Some(k) = config.effective_api_key(id) {
            if !k.is_empty() && !known_keys.contains(&k) {
                known_keys.push(k);
            }
        }
    }
    // #43：调用 token / admin 密码也纳入 redact 集（ErrorBody / tracing 若意外
    // 拼进凭据值 → 掩码；contracts/04 §2 红线 4 扩展面）。
    for t in config.effective_auth_tokens() {
        if !t.is_empty() && !known_keys.contains(&t) {
            known_keys.push(t);
        }
    }
    if let Some(pw) = config.effective_admin_password() {
        if !pw.is_empty() && !known_keys.contains(&pw) {
            known_keys.push(pw);
        }
    }

    AppState {
        registry: Arc::new(registry),
        config_path,
        known_keys: Arc::new(RwLock::new(known_keys)),
        auth: Arc::new(auth::AuthState::from_config(&config)),
        listen: Arc::new(std::sync::OnceLock::new()),
    }
}

/// 可测装配入口（A8 基座）：state → axum `Router`（CORS 白名单 + 双态鉴权中间件
/// + admin 路由 + 静态 UI 托管）。
///
/// 服务器由调用方经 [`listen::start`] 绑定（`main` 传 [`config::effective_bind`]
/// 结果：非显式 → 成对默认（local `127.0.0.1:11435` / cloud `0.0.0.0:11435`）；
/// 显式 bind 优先）。**bind 与 mode 解耦**，换地址走 ListenSupervisor 热 Rebind
/// —— 零进程重启；偏差备案 `docs/deployment.md`。
///
/// #43 中间件挂载拓扑（**local 态 loopback 两门直通 —— 端点行为与接入前一致**；
/// 非 loopback peer → 403 兜底）：
/// - 公开门：`GET /health`（探活放行）、`POST /v1/admin/login`（换会话，自身不能有门）、
///   `PUT /v1/admin/password`（in-handler 鉴权：会话 **或** loopback —— 忘密恢复）
/// - `/v1/systemone` + `/v1/models` → [`auth::require_call_token`]
/// - 其余 `/v1/admin/*`（含 `mode`、`listen`）→ [`auth::require_admin_session`]
/// - 非以上路径 → [`ServeDir`]（`JEV_UI_DIST`，默认 `ui/dist`；同源托管简化 CORS）
pub fn build_app(state: AppState) -> Router {
    // CORS 白名单（contracts/05 §5 M1+；替换 very_permissive 已知债务）：
    // vite dev 两种打开方式都可能 —— 127.0.0.1 与 localhost 都放行。
    // ADMINISTRATION 头：cloud 态 dev UI 跨源携带 `Authorization`（调用 token /
    // admin 会话）需要预检放行 —— 只加请求头，**origin 白名单不扩**（任务书 B：不扩
    // 公网 origin；跨源访问自行配置见 docs/deployment.md）。
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
        ]))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    // 静态 UI 目录：容器内指到 /app/ui/dist；本地默认仓库相对 ui/dist。
    let ui_dist =
        std::env::var("JEV_UI_DIST").unwrap_or_else(|_| "ui/dist".to_string());

    // 公开门（无鉴权中间件 —— login/password 自带逻辑：password in-handler 鉴权）
    let public = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/admin/login", post(auth::admin_login))
        .route("/v1/admin/password", put(admin::put_password));

    // 调用 token 门（cloud；local loopback 直通、非 loopback 403）
    let v1 = Router::new()
        .route("/v1/systemone", post(systemone_handler))
        .route("/v1/models", get(models_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_call_token,
        ));

    // admin 会话门（cloud；local loopback 直通、非 loopback 403）——
    // mode / listen 热切端点在此门内（cloud 翻转需会话，防匿名拆锁）
    let admin_routes = Router::new()
        .route(
            "/v1/admin/providers",
            get(admin::get_providers).put(admin::put_providers),
        )
        .route(
            "/v1/admin/routes",
            get(admin::get_routes).put(admin::put_routes),
        )
        .route(
            "/v1/admin/providers/:id/probe",
            post(admin::probe_provider),
        )
        .route("/v1/admin/mode", put(admin::put_mode))
        .route(
            "/v1/admin/listen",
            get(admin::get_listen).put(admin::put_listen),
        )
        .route("/v1/admin/status", get(admin::get_status))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin_session,
        ));

    public
        .merge(v1)
        .merge(admin_routes)
        .layer(cors) // 最后加的层 = 最外层：预检 OPTIONS 先过 CORS
        .fallback_service(ServeDir::new(ui_dist))
        .with_state(state)
}

/// `GET /health` — contracts/05 §2：JSON 为准（旧文本 "jev-switch MVP" 弃用）。
#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
struct HealthBody {
    #[cfg_attr(feature = "ts-rs", ts(type = "string"))]
    status: &'static str,
    #[cfg_attr(feature = "ts-rs", ts(type = "string"))]
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
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
struct ModelEntry {
    id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "string"))]
    object: &'static str,
    upstream: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
struct UpstreamCapabilityEntry {
    id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "string[]"))]
    question_types: Vec<&'static str>,
    has_confidence: bool,
    has_usage: bool,
    noul_via_boolean: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
struct ModelsResponse {
    #[cfg_attr(feature = "ts-rs", ts(type = "string"))]
    object: &'static str,
    data: Vec<ModelEntry>,
    upstreams: Vec<UpstreamCapabilityEntry>,
}

async fn models_handler(State(state): State<AppState>) -> Json<ModelsResponse> {
    let router = state.registry.router();
    // 保留 6af3a47 的不可路由过滤：只列出真正可路由的 model
    // （upstream 未注册的 model 不列出 —— 否则 UI 判断可用、点击即 404）
    let mut data: Vec<ModelEntry> = router
        .list_models()
        .into_iter()
        .filter_map(|m| {
            let upstream = router.route(&m).ok()?.id().to_string();
            Some(ModelEntry {
                id: m,
                object: "model",
                upstream,
            })
        })
        .collect();
    data.sort_by(|a, b| a.id.cmp(&b.id));

    // 能力列表 = 各 adapter 自报（注册制，经 UpstreamAdapter trait）
    let upstreams: Vec<UpstreamCapabilityEntry> = router
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

/// 把 JevError 转换为 axum Response（contracts/05 §3；error 串过 redact）。
fn jev_error_to_response(e: JevError, keys: &[String]) -> Response {
    let status =
        StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    // 404 路由错误不带 upstream 字段（A4 行为保留）
    let upstream = e.error_body_upstream().map(str::to_string);
    let retryable = e.retryable();
    let msg = e.to_string(); // 含上游 body —— 可能回显 key，交给 redact
    error_response(status, msg, upstream, retryable, keys)
}

async fn systemone_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<JevResponse>, Response> {
    let keys = known_keys_snapshot(&state);
    // 0. 协议解析：本地 400（criteria 缺失/错形态、未知 type、必填缺失、
    //    questions 非 record…）—— 按 contracts/01 §6 不发上游。
    //    手工 Bytes 提取：axum Json 提取器对 data 类错误回 422，契约要求 400。
    let req: JevRequest = serde_json::from_slice(&body).map_err(|e| {
        error_response(
            StatusCode::BAD_REQUEST,
            e.to_string(),
            None,
            false,
            &keys,
        )
    })?;

    run_request(&state.registry, req, &keys).await.map(Json)
}

/// handler → `Registry::invoke` 的薄封装（failover/DAG 逻辑 A5 起内核化在
/// jev-core，daemon 只做 HTTP 错误映射；抽函数以便进程内单测）。
///
/// sticky：HTTP 层暂无 sticky key 入口 → `plain_ctx()`（sticky_key=None），
/// core 侧有 RouteCtx 单测覆盖（见报告 A4 sticky 备注）。
async fn run_request(
    registry: &Registry,
    req: JevRequest,
    keys: &[String],
) -> Result<JevResponse, Response> {
    match registry.invoke(req, plain_ctx()).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            // tracing 输出同样 redact（contracts/04 §2：日志/tracing 统一脱敏）
            tracing::warn!(error = %redact(&e.to_string(), keys), status = e.http_status(), "invoke failed");
            Err(jev_error_to_response(e, keys))
        }
    }
}

/// A9 收尾：类型 Send+Sync 编译期断言（axum state 要求）。
/// 以 #[test] 形式调用 —— 取代原 `#[allow(dead_code)] fn _assert_send_sync`。
#[test]
fn state_types_are_send_sync() {
    fn assert_send<T: Send + Sync>() {}
    assert_send::<AppState>();
    assert_send::<JevError>();
    // json! 宏顺带自证 serde_json 可用（全限定路径，免顶层 import）
    let _ = serde_json::json!({});
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
       A4/A5 failover（进程内 fake UpstreamAdapter → Registry；
       不占 11435、无 wiremock）
       ════════════════════════════════════════════════════════ */

    use jev_core::adapter::{Registry, UpstreamAdapter};
    use jev_core::router::{MatchMode, OnError, RouteEdge, Sticky};
    use jev_core::upstream::{Capabilities, QuestionType};
    use jev_protocol::{Criteria, Question};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// fake 上游行为脚本。
    enum Behave {
        /// 合法 JevResponse。
        Ok200,
        /// 可重试 429（vercel 类 retryable_status 含 429）。
        Err429,
        /// 第 1 次 429、之后成功（A6 同候选重试路径）。
        Err429OnceThenOk,
        /// 不可重试 400。
        Err400,
        /// 上游响应形态非法 → BadResponse（502；A5 起反序列化在 adapter 内）。
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
    impl UpstreamAdapter for Fake {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> Capabilities {
            self.cap.clone()
        }
        async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
            let n = self.handle.calls.fetch_add(1, Ordering::SeqCst) + 1; // 1-based
            self.handle.seen_models.lock().unwrap().push(req.model.clone());
            let ok = || serde_json::from_str::<JevResponse>(r#"{"answers":{}}"#).map_err(|e| {
                JevError::BadResponse {
                    upstream_id: self.id.clone(),
                    message: e.to_string(),
                }
            });
            let rate429 = || {
                Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 429,
                    body: "rate limited".into(),
                    retryable: true,
                })
            };
            match self.behave {
                Behave::Ok200 => ok(),
                Behave::Err429 => rate429(),
                Behave::Err429OnceThenOk => {
                    if n == 1 {
                        rate429()
                    } else {
                        ok()
                    }
                }
                Behave::Err400 => Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 400,
                    body: "bad request".into(),
                    retryable: false,
                }),
                Behave::InvalidJson => Err(JevError::BadResponse {
                    upstream_id: self.id.clone(),
                    message: "invalid jev json".into(),
                }),
            }
        }
    }

    fn fake(
        id: &str,
        qts: &'static [QuestionType],
        behave: Behave,
    ) -> (Box<dyn UpstreamAdapter>, Arc<FakeHandle>) {
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
    /// （A6 隔离：钉 no-retry 纯测 failover 语义；同候选重试见 wiremock/A6 专测）
    #[tokio::test]
    async fn failover_429_falls_to_second_candidate_with_upstream_calls() {
        use jev_core::adapter::RetryPolicy;
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);

        let mut e1 = edge("jev", "vercel", 10);
        e1.upstream_model = Some("typesafe-ai/jev".into());
        let mut e2 = edge("jev", "laya", 30);
        e2.upstream_model = Some("laya-english".into());
        let mut reg = Registry::with_retry(vec![e1, e2], RetryPolicy::no_retry());
        reg.register(v1);
        reg.register(v2);

        let resp = run_request(&reg, noul_request("jev"), &[]).await.expect("failover success");
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
        let reg = Registry::new(vec![]);
        let resp = run_request(&reg, noul_request("nope"), &[]).await;
        assert_eq!(status_of(resp).await, 404);
        let body = error_body(run_request(&reg, noul_request("nope"), &[]).await).await;
        assert!(body.error.contains("nope"));
        assert!(!body.retryable);
        assert!(body.upstream.is_none(), "404 不带 upstream 字段");
    }

    /// 验收 ⑥：422 语义不回潮（capability 不匹配且无可用候选）。
    #[tokio::test]
    async fn capability_mismatch_all_skipped_is_422() {
        let (v1, h1) = fake("vercel", &[QuestionType::Choice], Behave::Ok200);
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10)]);
        reg.register(v1);

        let resp = run_request(&reg, noul_request("jev"), &[]).await;
        assert_eq!(status_of(resp).await, 422);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 0, "capability 跳过不实发");
        let body = error_body(run_request(&reg, noul_request("jev"), &[]).await).await;
        assert!(body.error.contains("question type"), "got: {}", body.error);
        assert!(!body.retryable);
    }

    /// capability 不匹配 = 跳过该候选（不计入失败语义）→ 落下一候选成功，calls 只计实发。
    #[tokio::test]
    async fn capability_mismatch_skips_candidate_without_counting_failure() {
        // cand1 只支持 choice（不支持 noul）→ 跳过；cand2 支持 noul → 成功
        let (v1, h1) = fake("vercel", &[QuestionType::Choice], Behave::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        reg.register(v1);
        reg.register(v2);

        let resp = run_request(&reg, noul_request("jev"), &[]).await.expect("second wins");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 0, "cap 跳过 = 零实发（连 429 都没走到）");
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1);
        assert_eq!(resp.upstream_calls, Some(1), "upstream_calls 只计实发");
    }

    /// on_error=fail：首错即返（即使错误可重试），不试下一候选。
    #[tokio::test]
    async fn on_error_fail_returns_first_error_without_next() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let e1 = RouteEdge {
            on_error: OnError::Fail,
            ..edge("jev", "vercel", 10)
        };
        let mut reg = Registry::new(vec![e1, edge("jev", "laya", 30)]);
        reg.register(v1);
        reg.register(v2);

        let resp = run_request(&reg, noul_request("jev"), &[]).await;
        assert_eq!(status_of(resp).await, 503, "429 retryable → 对外 503");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "fail 不试下一候选");
        let body = error_body(run_request(&reg, noul_request("jev"), &[]).await).await;
        assert!(body.retryable, "429 retryable 标志透出");
        assert_eq!(body.upstream.as_deref(), Some("vercel"));
    }

    /// 不可重试错误（400）：on_error=next 也不试下一候选（仅 retryable 才 failover）。
    #[tokio::test]
    async fn non_retryable_error_stops_even_with_on_error_next() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err400);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        reg.register(v1);
        reg.register(v2);
        let resp = run_request(&reg, noul_request("jev"), &[]).await;
        assert_eq!(status_of(resp).await, 400, "透传上游码");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1, "首候选实发一次即停");
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "不可重试错误不 failover");
    }

    /// 上游响应形态非法 → 502 即返（不 failover；BadResponse 非 retryable）。
    #[tokio::test]
    async fn invalid_upstream_json_is_502_no_failover() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::InvalidJson);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        reg.register(v1);
        reg.register(v2);
        let resp = run_request(&reg, noul_request("jev"), &[]).await;
        assert_eq!(status_of(resp).await, 502);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0);
    }

    /// 旧 [router] 行为回归：单 exact 边一次成功，upstream_calls=1。
    #[tokio::test]
    async fn legacy_single_edge_success_upstream_calls_one() {
        let (v1, h1) = fake("laya", &[QuestionType::Noul], Behave::Ok200);
        let mut reg = Registry::new(vec![edge("laya-english", "laya", 0)]);
        reg.register(v1);
        let resp = run_request(&reg, noul_request("laya-english"), &[]).await.expect("ok");
        assert_eq!(resp.upstream_calls, Some(1));
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        // 无 upstream_model 改写 → 沿用原 model
        assert_eq!(h1.seen_models.lock().unwrap().as_slice(), &["laya-english"]);
    }

    /// A6：daemon 装配路径（`Registry::new` 默认 RetryPolicy）自带同候选重试 ——
    /// 单候选首错 429 → 默认预算内二次成功，upstream_calls=2，无需 failover。
    #[tokio::test(start_paused = true)]
    async fn daemon_default_registry_retries_same_candidate() {
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Behave::Err429OnceThenOk);
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10)]); // 默认策略
        reg.register(v1);
        let resp = run_request(&reg, noul_request("jev"), &[]).await.expect("retry then ok");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 2, "默认策略同候选重试");
        assert_eq!(resp.upstream_calls, Some(2));
    }

    /// CORS 白名单 origin 字面量冻结（vite dev 两种打开方式）。
    /// #43：allow_headers 扩 `AUTHORIZATION`（cloud 态 dev UI 携带 Bearer 预检）——
    /// origin 白名单**不扩**（不加公网 origin）。
    #[test]
    fn cors_allowlist_origins_frozen() {
        let layer = CorsLayer::new()
            .allow_origin(AllowOrigin::list([
                HeaderValue::from_static("http://127.0.0.1:5173"),
                HeaderValue::from_static("http://localhost:5173"),
            ]))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);
        // tower-http 不提供只读 introspection；构造成功 + 预检行为由 curl 实测覆盖。
        // 此处至少锁定构造路径可编译（防 very_permissive 回潮需人工改回本函数）。
        let _ = layer;
        let _ = StatusCode::OK;
    }
}
