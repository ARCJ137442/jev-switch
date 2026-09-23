//! #43 双态鉴权（docs/12 §一/§二.6/§四线1 —— 裁决已锁，不重新设计）。
//!
//! - **local（默认，现状回归）**：loopback peer 两道中间件直接放行；
//!   **非 loopback peer → 403**（`local mode: loopback only`，三键错误体 ——
//!   显式 bind=0.0.0.0 场景的应用层兜底；非显式默认绑 127.0.0.1 内核级已挡）。
//! - **cloud**：
//!   - `/v1/systemone`、`/v1/models` → `Authorization: Bearer <调用 token>`
//!     （token 来自配置 `auth_tokens` 两种写法 / env `JEV_AUTH_TOKENS`；
//!     单账号语义，**不做 token 管理体系**）。缺失/错值 → **401** 统一错误体
//!     `{error, upstream:null, retryable:false}`（经 redact，contracts/05 §3 形状）。
//!   - `/health` **放行**（容器/探针探活，任务书字面）。
//!   - `/v1/admin/*` → 独立管理密码：`POST /v1/admin/login {password}` 换**短时会话
//!     token**（内存态、不落盘、过期即废）→ 后续 admin 请求带
//!     `Authorization: Bearer <会话 token>`。密码来源：配置 `admin_password`
//!     （明文 toml —— Q4=b 哲学）/ env `JEV_ADMIN_PASSWORD` 优先 /
//!     运行时热更 `PUT /v1/admin/{mode,password}`（统一 `admin::set_admin_password`）。
//!
//! **mode = 运行时策略**（热切裁决）：`AuthState.mode` 是 `RwLock<RunMode>`，
//! 中间件**每请求读锁**；`PUT /v1/admin/mode` 写锁翻转 → 后续请求立即生效
//! （在途请求跑完旧策略）。与 bind 解耦：非显式 bind 时翻转联动 ListenSupervisor
//! 热 Rebind（见 [`crate::listen`]）。
//!
//! **login 形态选择（任务书二选一 → 报告说明）**：取「登录换会话 token」标准形态，
//! 而非每请求 `X-Admin-Password` 头 —— 理由：① 密码只在网络出现一次，之后只有
//! 短时 token 参与传输，缩小密码暴露面；② 会话可过期/失效（改密码即全废 ——
//! 统一经 `set_admin_password` 代际作废），头形态每请求都是有效凭据；
//! ③ 浏览器侧只存 token 不存密码。
//!
//! 防偷（contracts/04 §2）：
//! - 密码/会话 token **绝不进日志**（本模块 tracing 只输出布尔/计数）；
//! - login 响应只有 `{token, expires_in}`，任何 GET 响应不含会话/密码；
//! - 会话 token 与调用 token 分池校验（**token 混用拒绝**：调用 token 进 admin → 401）。

use crate::{error_response, AppState};
use axum::{
    body::Bytes,
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// 会话有效期（「短时」裁决的具体化 —— 2 小时）。
pub const ADMIN_SESSION_TTL: Duration = Duration::from_secs(2 * 60 * 60);

/* ══════════════════════════════════════════════════════════════════
   AuthState（挂在 AppState.auth —— Arc 共享，逐请求克隆只碰指针）
   ══════════════════════════════════════════════════════════════════ */

/// 双态鉴权运行态（**不 derive Clone** —— 内含 `RwLock`；共享靠 `AppState.auth`
/// 的 `Arc<AuthState>`，克隆只碰指针）。
pub struct AuthState {
    /// 运行模式（**运行时策略**：启动由 `Config::effective_mode` 解析，
    /// `PUT /v1/admin/mode` 热翻转；中间件每请求读锁拷贝，guard 不跨 await）。
    pub mode: RwLock<crate::config::RunMode>,
    /// cloud `/v1` 调用 token 列表（local 态忽略）。
    pub auth_tokens: Vec<String>,
    /// cloud admin 登录密码（明文 —— 仅存在内存；None = fail-closed）。
    /// 运行时可经统一入口 `admin::set_admin_password` 热更（持久化+会话作废）。
    pub admin_password: RwLock<Option<String>>,
    /// admin 会话：token → **过期时刻**（内存态，不落盘；重启全废）。
    /// 存 `expires_at` 而非签发时刻 —— prune 判断不依赖 `now - created` 的
    /// 时钟回退语义（Instant 单调，但存到期点让测试可注入 1s 前的过期戳，
    /// 无「机器 uptime < TTL」假设）。
    /// 密码变更 → `set_admin_password` 直接 `clear()` + 代际计数器 +1。
    pub sessions: RwLock<HashMap<String, Instant>>,
    /// 启动期捕获：env `JEV_SWITCH_MODE` 非空 → `PUT /mode` 响应
    /// `env_override_active` 警示（下次启动 env 覆盖文件 mode）。
    pub env_mode_override: AtomicBool,
    /// 启动期捕获：env `JEV_ADMIN_PASSWORD` 非空 → 密码写文件后的警示
    /// （下次启动 env 覆盖回去）。测试可直接置位（免 env 并行竞态）。
    pub env_password_override: AtomicBool,
    /// 会话代际计数器：密码变更 +1（配合 `sessions.clear()`；诊断/断言用）。
    pub session_generation: AtomicU64,
}

impl Default for AuthState {
    /// local 默认（缺省零鉴权 —— 现状回归）。
    fn default() -> Self {
        AuthState {
            mode: RwLock::new(crate::config::RunMode::Local),
            auth_tokens: Vec::new(),
            admin_password: RwLock::new(None),
            sessions: RwLock::new(HashMap::new()),
            env_mode_override: AtomicBool::new(false),
            env_password_override: AtomicBool::new(false),
            session_generation: AtomicU64::new(0),
        }
    }
}

impl AuthState {
    /// 从配置（已含 env 覆盖解析）构造；cloud 态缺 token/密码时 fail-closed 并告警。
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        let mode = match cfg.effective_mode() {
            Ok(m) => m,
            Err(e) => {
                // 真实二进制路径 main 会先硬错拒启；此分支只兜直接 build_state 的调用方。
                tracing::error!(error = %e, "invalid JEV_SWITCH_MODE — falling back to file mode");
                cfg.mode
            }
        };
        let auth_tokens = cfg.effective_auth_tokens();
        let admin_password = cfg.effective_admin_password();
        if mode == crate::config::RunMode::Cloud {
            if auth_tokens.is_empty() {
                tracing::warn!(
                    "cloud mode: auth_tokens empty — all /v1 calls will be rejected (401, fail-closed)"
                );
            }
            if admin_password.is_none() {
                tracing::warn!(
                    "cloud mode: admin_password unset — admin login disabled (fail-closed)"
                );
            }
        }
        // 注意：只记数量，**绝不打印 token/密码值**。
        tracing::info!(?mode, call_tokens = auth_tokens.len(), admin_password_set = admin_password.is_some(), "auth initialized");
        AuthState {
            mode: RwLock::new(mode),
            auth_tokens,
            admin_password: RwLock::new(admin_password),
            sessions: RwLock::new(HashMap::new()),
            env_mode_override: AtomicBool::new(env_nonempty("JEV_SWITCH_MODE")),
            env_password_override: AtomicBool::new(env_nonempty("JEV_ADMIN_PASSWORD")),
            session_generation: AtomicU64::new(0),
        }
    }
}

/// env 非空判定（空串/未设 = false；启动期捕获一次，避免请求期读 env 的
/// 测试并行竞态 —— 进程生命周期内 env 视作恒定）。
fn env_nonempty(key: &str) -> bool {
    std::env::var(key)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/* ══════════════════════════════════════════════════════════════════
   内部工具
   ══════════════════════════════════════════════════════════════════ */

/// 常数时间字节比较（长度不等直接 false —— 长度侧信道可接受）。
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 提取 `Authorization: Bearer <token>`（scheme 大小写不敏感；缺失/怪形 → None）。
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = v.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let t = token.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// 当前运行模式（读锁拷贝 —— guard 立即 drop，**绝不跨 await 持有**）。
pub(crate) fn current_mode(state: &AppState) -> crate::config::RunMode {
    *state.auth.mode.read().expect("mode lock")
}

/// loopback 判定。**peer 缺失（None）按 loopback 信任** —— 理由：真实 TCP
/// 必有 `ConnectInfo`（`into_make_service_with_connect_info` 注入）；None 仅
/// 出现在进程内 oneshot/单元测试（无 TCP 对端），按本机信任保持
/// `local_mode_requires_no_auth_anywhere` 既有语义。
pub(crate) fn addr_is_loopback(addr: Option<&std::net::SocketAddr>) -> bool {
    addr.map_or(true, |a| a.ip().is_loopback())
}

/// 从请求扩展取 `ConnectInfo<SocketAddr>` 判 loopback（中间件路径）。
pub(crate) fn peer_is_loopback(req: &axum::extract::Request) -> bool {
    use axum::extract::ConnectInfo;
    let addr = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ConnectInfo(a)| a);
    addr_is_loopback(addr)
}

/// 401 统一错误体（contracts/05 §3 形状；经 redact —— 红线 4）。
fn unauthorized(state: &AppState, msg: &str) -> Response {
    let keys = crate::known_keys_snapshot(state);
    error_response(StatusCode::UNAUTHORIZED, msg, None, false, &keys)
}

/// 401（admin 域公开复用 —— `PUT /v1/admin/password` in-handler 鉴权用）。
pub(crate) fn unauthorized_pub(state: &AppState, msg: &str) -> Response {
    unauthorized(state, msg)
}

/// 403 统一错误体（local 态非 loopback peer 兜底 —— 三键同 401 形状）。
pub(crate) fn forbidden(state: &AppState, msg: &str) -> Response {
    let keys = crate::known_keys_snapshot(state);
    error_response(StatusCode::FORBIDDEN, msg, None, false, &keys)
}

/// 会话有效性（prune 过期 + 查表）。`token=None` → false。
/// 中间件与 `PUT /v1/admin/password` 的 in-handler 鉴权共用同一入口。
pub(crate) fn has_valid_session(state: &AppState, token: Option<&str>) -> bool {
    let Some(tok) = token else { return false };
    let mut sessions = state.auth.sessions.write().expect("admin sessions lock");
    prune_expired(&mut sessions);
    sessions.contains_key(tok)
}

/* ══════════════════════════════════════════════════════════════════
   中间件（local 直通 + peer 兜底 / cloud 校验 —— 每请求读锁取 mode）
   ══════════════════════════════════════════════════════════════════ */

/// `/v1/systemone`、`/v1/models` 的调用 token 门。
pub async fn require_call_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if current_mode(&state) == crate::config::RunMode::Local {
        // local：loopback 全放行（现状语义）；非 loopback → 403 应用层兜底
        // （显式 bind=0.0.0.0 时 LAN 可达的残留保护；非显式默认已绑 127.0.0.1）
        if !peer_is_loopback(&req) {
            return forbidden(&state, "local mode: loopback only");
        }
        return next.run(req).await;
    }
    match bearer_token(req.headers()) {
        Some(tok)
            if state
                .auth
                .auth_tokens
                .iter()
                .any(|t| ct_eq(t.as_bytes(), tok.as_bytes())) =>
        {
            next.run(req).await
        }
        _ => unauthorized(&state, "missing or invalid bearer token"),
    }
}

/// `/v1/admin/*` 的会话门（login/password 端点本身不走此门 —— build_app 挂在门外）。
pub async fn require_admin_session(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if current_mode(&state) == crate::config::RunMode::Local {
        if !peer_is_loopback(&req) {
            return forbidden(&state, "local mode: loopback only");
        }
        return next.run(req).await; // 现状回归：loopback 零校验
    }
    // 锁作用域在 has_valid_session 内完成 prune + 查验 —— `RwLockWriteGuard` 是
    // !Send，绝不跨越 `next.run(...).await`（否则整个 middleware future 非 Send）。
    if has_valid_session(&state, bearer_token(req.headers())) {
        next.run(req).await
    } else {
        unauthorized(&state, "admin session required (POST /v1/admin/login)")
    }
}

/// 丢弃过期会话（每次写锁进入时调用 —— 惰性清理，无后台任务）。
/// 值 = `expires_at`；`now < expires_at` 才存活。
fn prune_expired(sessions: &mut HashMap<String, Instant>) {
    let now = Instant::now();
    sessions.retain(|_, expires_at| *expires_at > now);
}

/* ══════════════════════════════════════════════════════════════════
   POST /v1/admin/login
   ══════════════════════════════════════════════════════════════════ */

/// login 请求体（**只 Deserialize** —— 密码永不进任何序列化面，同 ProviderInput 纪律）。
#[derive(Debug, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct AdminLoginRequest {
    pub password: String,
}

/// login 响应体（唯一回传会话 token 的位置；不含密码）。
#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct AdminLoginResponse {
    /// 短时会话 token（内存态；后续 `Authorization: Bearer <token>`）。
    pub token: String,
    /// 有效期秒数（= [`ADMIN_SESSION_TTL`]）。
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub expires_in: u64,
}

/// 会话 token 生成：`RandomState`（OS 播种）双实例混合
/// (纳秒时钟, 进程 id, 进程内计数器) → 2×u64 → 32 hex 字符。
/// 零新依赖；对「内存短会话」强度足够（非常数时间 token 生成的 CSPRNG 要求，
/// 用途等价于一次性随机 nonce —— 备案见 #43 报告）。
fn new_session_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id() as u64;
    let h1 = {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(nanos);
        h.write_u64(pid);
        h.write_u64(n);
        h.finish()
    };
    let h2 = {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(h1);
        h.write_u64(n);
        h.write_u64(nanos ^ pid);
        h.finish()
    };
    format!("{h1:016x}{h2:016x}")
}

/// `POST /v1/admin/login {password}` → `{token, expires_in}`。
///
/// - 未配置密码（cloud fail-closed / local 未设）→ 401，**不区分**「未配置/密码错」
///   以外的更多信息（错误文案固定，防枚举；未配置另有启动 warn 指路）。
/// - 密码错误 → 401 统一错误体。
/// - 成功 → 200 + 会话 token（唯一回传点）。
pub async fn admin_login(State(state): State<AppState>, body: Bytes) -> Response {
    let req: AdminLoginRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                format!("invalid login body: {e}"),
                None,
                false,
                &[],
            )
        }
    };

    // 读锁拷贝密码（RwLock guard 不跨后续逻辑；值不进日志）
    let configured = state.auth.admin_password.read().expect("password lock").clone();
    let Some(configured) = configured else {
        // fail-closed：无密码配置 → 拒绝一切登录（不回显配置细节）。
        tracing::warn!("admin login attempt rejected: admin_password not configured");
        return unauthorized(&state, "admin login unavailable");
    };

    if !ct_eq(configured.as_bytes(), req.password.as_bytes()) {
        tracing::warn!("admin login failed: password mismatch"); // 不打印密码
        return unauthorized(&state, "invalid admin password");
    }

    let token = new_session_token();
    {
        let mut sessions = state
            .auth
            .sessions
            .write()
            .expect("admin sessions lock");
        prune_expired(&mut sessions);
        sessions.insert(token.clone(), Instant::now() + ADMIN_SESSION_TTL);
    }
    tracing::info!("admin login ok"); // 无任何凭据值
    Json(AdminLoginResponse {
        token,
        expires_in: ADMIN_SESSION_TTL.as_secs(),
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_app, build_state, config::Config};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// 测试假密码/假 token（真实值永不进测试/提交）。
    const FAKE_PW: &str = "pw-test-admin-1234";
    const FAKE_TOK: &str = "tok-test-call-aaaa";

    fn temp_config(name: &str, content: &str) -> std::path::PathBuf {
        // 计数器后缀：同进程并行测试不能共享目录（否则一个的 cleanup 会删掉
        // 另一个正在 load 的文件 —— 本模块初版实测 4 测同名互踩全红）。
        static N: AtomicU64 = AtomicU64::new(0);
        let seq = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "jev-auth-{}-{}-{}",
            name,
            std::process::id(),
            seq
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("providers.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    fn cloud_cfg(extra: &str) -> String {
        format!(
            r#"
mode = "cloud"
auth_tokens = ["{FAKE_TOK}"]
admin_password = "{FAKE_PW}"
{extra}
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
        )
    }

    fn app_of(content: &str) -> (axum::Router, std::path::PathBuf) {
        let path = temp_config("t", content);
        let cfg = Config::load(&path).expect("load temp config");
        let app = build_app(build_state(cfg, path.clone()));
        (app, path)
    }

    async fn send(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Option<String>,
        bearer: Option<&str>,
    ) -> (u16, String) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(t) = bearer {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        let req = match body {
            Some(b) => builder.body(Body::from(b)).unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    fn cleanup(path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /* ── local 回归：一切照旧，零鉴权 ─────────────────────────────── */

    #[tokio::test]
    async fn local_mode_requires_no_auth_anywhere() {
        // 缺省配置 = mode local（不写 mode 字段）
        let (app, path) = app_of(
            r#"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#,
        );
        // /v1/models 无 token → 200
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, None).await;
        assert_eq!(s, 200, "local /v1/models 免 token: {b}");
        // admin GET 无会话 → 200
        let (s, b) = send(app.clone(), "GET", "/v1/admin/providers", None, None).await;
        assert_eq!(s, 200, "local admin 免会话: {b}");
        // systemone 无 token：必须走到 handler（未知 model → 404），**绝不是 401**
        let body = serde_json::json!({
            "model": "ghost-model", "state": "t",
            "questions": {"q": {"type": "noul", "instructions": "x",
                "criteria": {"true": "y", "false": "n"}}}
        })
        .to_string();
        let (s, b) = send(app.clone(), "POST", "/v1/systemone", Some(body), None).await;
        assert_eq!(s, 404, "local systemone 零鉴权干扰（404 而非 401）: {b}");
        // health 照旧
        let (s, _) = send(app, "GET", "/health", None, None).await;
        assert_eq!(s, 200);
        cleanup(&path);
    }

    /* ── cloud /v1：401 形状 + 正确 token 200 + health 放行 ──────── */

    #[tokio::test]
    async fn cloud_v1_missing_or_wrong_token_401_with_frozen_error_body() {
        let (app, path) = app_of(&cloud_cfg(""));
        // 缺失
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, None).await;
        assert_eq!(s, 401, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        let obj = v.as_object().expect("401 是对象");
        assert_eq!(obj.len(), 3, "错误体恰三键: {b}");
        assert!(v["error"].is_string(), "{b}");
        assert_eq!(v["upstream"], serde_json::Value::Null, "{b}");
        assert_eq!(v["retryable"], false, "{b}");
        // 错值（形态合法的假 token）
        let (s, b) = send(
            app.clone(),
            "GET",
            "/v1/models",
            None,
            Some("tok-test-wrong-zzzz"),
        )
        .await;
        assert_eq!(s, 401, "{b}");
        // systemone 缺 token → 401（中间件先于协议解析）
        let body = serde_json::json!({
            "model": "ghost", "state": "t",
            "questions": {"q": {"type": "noul", "instructions": "x",
                "criteria": {"true": "y", "false": "n"}}}
        })
        .to_string();
        let (s, b) = send(app.clone(), "POST", "/v1/systemone", Some(body), None).await;
        assert_eq!(s, 401, "cloud systemone 无 token 必须 401: {b}");
        // 正确 token → 200
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, Some(FAKE_TOK)).await;
        assert_eq!(s, 200, "正确调用 token → 200: {b}");
        // health 放行（无 token）
        let (s, b) = send(app, "GET", "/health", None, None).await;
        assert_eq!(s, 200, "health 探活放行: {b}");
        cleanup(&path);
    }

    /* ── cloud admin：login 全流程 + 分池 + 防偷 ──────────────────── */

    #[tokio::test]
    async fn cloud_admin_login_session_flow_and_no_credential_leak() {
        let (app, path) = app_of(&cloud_cfg(""));

        // ① admin 无会话 → 401
        let (s, b) = send(app.clone(), "GET", "/v1/admin/providers", None, None).await;
        assert_eq!(s, 401, "{b}");

        // ② login 密码错 → 401（错误体形状）
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(r#"{"password":"pw-test-wrong"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 401, "{b}");
        assert!(!b.contains("pw-test-wrong"), "错误回显不得含提交的密码: {b}");

        // ③ login 密码缺字段 → 400
        let (s, _) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(r#"{}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 400);

        // ④ login 成功 → 200 {token, expires_in}；响应不含密码
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(format!(r#"{{"password":"{FAKE_PW}"}}"#)),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        assert!(!b.contains(FAKE_PW), "login 响应泄露密码: {b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        let session = v["token"].as_str().expect("token 字段").to_string();
        assert!(!session.is_empty());
        assert_ne!(session, FAKE_PW, "会话 token 不得等于密码");
        assert_eq!(v["expires_in"], ADMIN_SESSION_TTL.as_secs());

        // ⑤ 带会话 → admin 200，且 GET 响应不含密码/会话值（防偷复核）
        let (s, b) = send(
            app.clone(),
            "GET",
            "/v1/admin/providers",
            None,
            Some(&session),
        )
        .await;
        assert_eq!(s, 200, "{b}");
        assert!(!b.contains(FAKE_PW), "admin GET 泄露密码: {b}");
        assert!(!b.contains(&session), "admin GET 泄露会话 token: {b}");

        // ⑥ **分池**：调用 token 拒于 admin 门外
        let (s, b) = send(app.clone(), "GET", "/v1/admin/providers", None, Some(FAKE_TOK)).await;
        assert_eq!(s, 401, "调用 token 不得当 admin 会话用: {b}");

        // ⑦ 分池反向：会话 token 拒于 /v1 门外
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, Some(&session)).await;
        assert_eq!(s, 401, "会话 token 不得当调用 token 用: {b}");

        cleanup(&path);
    }

    /* ── cloud admin：无密码配置 → fail-closed ───────────────────── */

    #[tokio::test]
    async fn cloud_admin_without_password_fails_closed() {
        let content = r#"
mode = "cloud"
auth_tokens = ["tok-test-call-bbbb"]
# 不配 admin_password
"#;
        let (app, path) = app_of(content);
        // login 任意密码 → 401（fail-closed）
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(r#"{"password":"whatever"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 401, "{b}");
        // admin GET 永远拿不到会话 → 401
        let (s, _) = send(app, "GET", "/v1/admin/routes", None, None).await;
        assert_eq!(s, 401);
        cleanup(&path);
    }

    /* ── 会话过期：惰性 prune ─────────────────────────────────────── */

    #[tokio::test]
    async fn expired_admin_session_is_rejected_and_pruned() {
        let path = temp_config("exp", &cloud_cfg(""));
        let cfg = Config::load(&path).unwrap();
        let state = build_state(cfg, path.clone());
        // 手工塞一枚「已过期」会话：expires_at = now - 1s（1s 早于任何运行中进程，
        // 无 uptime ≥ TTL 假设）
        let expired = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("monotonic clock allows 1s back");
        state
            .auth
            .sessions
            .write()
            .unwrap()
            .insert("deadbeefdeadbeef".into(), expired);
        let app = build_app(state.clone());

        let (s, b) = send(app, "GET", "/v1/admin/providers", None, Some("deadbeefdeadbeef")).await;
        assert_eq!(s, 401, "过期会话必须 401: {b}");
        // 惰性清理：过期条目已被 prune 掉
        assert!(state.auth.sessions.write().unwrap().is_empty());
        cleanup(&path);
    }

    /* ── 会话 token 生成：唯一性 ─────────────────────────────────── */

    #[test]
    fn session_tokens_are_unique() {
        let a = new_session_token();
        let b = new_session_token();
        assert_eq!(a.len(), 32, "2×u64 hex");
        assert_ne!(a, b, "两次生成必须不同");
    }

    /* ── ct_eq / bearer 解析 ─────────────────────────────────────── */

    #[test]
    fn ct_eq_and_bearer_parsing() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));

        let mk = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::AUTHORIZATION, v.parse().unwrap());
            bearer_token(&m).map(String::from)
        };
        assert_eq!(mk("Bearer tok-1").as_deref(), Some("tok-1"));
        assert_eq!(mk("bearer tok-2").as_deref(), Some("tok-2"));
        assert_eq!(mk("Basic dXNlcjpwdw=="), None);
        assert_eq!(mk("Bearer   "), None);
        assert_eq!(mk("Bearer"), None); // 无空格分隔
    }

    /* ════════════════════════════════════════════════════════════
       mode 热切 / 激活设密 / env 警示（oneshot —— None peer = loopback 信任）
       ════════════════════════════════════════════════════════════ */

    /// 带 state 句柄的装配（需要拨 env 警示原子位 / 查 RwLock）。
    fn app_state_of(content: &str) -> (axum::Router, crate::AppState, std::path::PathBuf) {
        let path = temp_config("modesw", content);
        let cfg = Config::load(&path).expect("load temp config");
        let state = build_state(cfg, path.clone());
        let app = build_app(state.clone());
        (app, state, path)
    }

    /// local 起步、无密码配置（激活 cloud 必须带第一个密码的场景）。
    fn local_no_pw_cfg() -> String {
        format!(
            r#"
mode = "local"
auth_tokens = ["{FAKE_TOK}"]
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
        )
    }

    #[tokio::test]
    async fn activate_cloud_without_password_is_400_with_guidance() {
        let (app, _state, path) = app_state_of(&local_no_pw_cfg());

        // 无密码激活 cloud → 400 + 指引文案（fail-closed 不破）
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"cloud"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 400, "{b}");
        assert!(
            b.contains("admin password required to activate cloud")
                && b.contains("admin_password"),
            "400 文案须含指引: {b}"
        );
        // 拒绝后：仍是 local（/v1/models 免 token 200），文件未写 mode
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, None).await;
        assert_eq!(s, 200, "拒绝激活后模式不得翻转: {b}");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("mode = \"cloud\""), "400 不得落盘: {on_disk}");

        // 非法 mode 字面 → 400（带病不上线）
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"prod"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 400, "{b}");
        // 空密码 → 400
        let (s, b) = send(
            app,
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"cloud","admin_password":"  "}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 400, "{b}");
        cleanup(&path);
    }

    #[tokio::test]
    async fn activate_cloud_with_password_bootstraps_atomically() {
        let (app, state, path) = app_state_of(&local_no_pw_cfg());

        // 一发带密码激活：写 toml(mode+admin_password) + 运行时生效 + 激活 cloud
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"cloud","admin_password":"pw-test-boot-1"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["mode"], "cloud");
        assert_eq!(v["persisted"], true);
        assert_eq!(v["env_override_active"], false, "默认无 env 覆盖: {b}");
        assert_eq!(
            v["rebind"]["skipped"], "no listener",
            "oneshot 无 supervisor → skipped: {b}"
        );
        assert!(!b.contains("pw-test-boot-1"), "响应不得回显密码: {b}");

        // 落盘核对
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("mode = \"cloud\""), "{on_disk}");
        assert!(on_disk.contains("admin_password = \"pw-test-boot-1\""), "{on_disk}");

        // 立即生效：/v1 现在要 token；login 用新密码拿会话
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, None).await;
        assert_eq!(s, 401, "激活后立即 401: {b}");
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, Some(FAKE_TOK)).await;
        assert_eq!(s, 200, "调用 token 放行: {b}");
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(r#"{"password":"pw-test-boot-1"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "激活时设的密码即可登录: {b}");
        let session = serde_json::from_str::<serde_json::Value>(&b).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();
        let (s, b) = send(
            app.clone(),
            "GET",
            "/v1/admin/providers",
            None,
            Some(&session),
        )
        .await;
        assert_eq!(s, 200, "cloud + 会话 → admin 200: {b}");

        // 代际：激活前若已有会话则作废 —— 本例无预存会话，直接验证写入后 generation 语义：
        assert_eq!(
            state.auth.session_generation.load(Ordering::SeqCst),
            1,
            "设密路径代际 +1"
        );

        // 激活后 status：cloud 态 401（无会话）→ 200（带会话）且 password_set=true
        let (s, b) = send(app.clone(), "GET", "/v1/admin/status", None, None).await;
        assert_eq!(s, 401, "cloud status 无会话 401（三键）: {b}");
        let obj: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(obj.as_object().unwrap().len(), 3, "{b}");
        let (s, b) = send(
            app.clone(),
            "GET",
            "/v1/admin/status",
            None,
            Some(&session),
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["mode"], "cloud", "{b}");
        assert_eq!(v["password_set"], true, "激活设密后必须 true: {b}");
        assert!(!b.contains("pw-test-boot-1"), "status 泄露密码: {b}");
        cleanup(&path);
    }

    #[tokio::test]
    async fn activation_picks_up_password_added_externally_to_file() {
        let (app, _state, path) = app_state_of(&local_no_pw_cfg());
        // 外部手改文件补密码（Q5 文件真值；**前置**插入根表 —— 追加会掉进 [router]）
        let original = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            format!("admin_password = \"pw-test-external-7\"\n{original}"),
        )
        .unwrap();

        // 激活 cloud：body 不带密码也应成功（运行时 None → 同步文件值 → 非从未配置）。
        // 此路径同时压「读 guard 跨 if 块再写锁」的死锁回归（修复前会挂死）。
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"cloud"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "文件已有密码 → 免带激活: {b}");

        // 用文件密码登录证明运行时已同步
        let (s, b) = send(
            app,
            "POST",
            "/v1/admin/login",
            Some(r#"{"password":"pw-test-external-7"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "同步后的文件密码可登录: {b}");
        cleanup(&path);
    }

    #[tokio::test]
    async fn mode_switch_with_new_password_rotates_and_kills_old_sessions() {
        let content = format!(
            r#"
mode = "local"
auth_tokens = ["{FAKE_TOK}"]
admin_password = "{FAKE_PW}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
        );
        let (app, state, path) = app_state_of(&content);

        // 旧密码登录 → 会话可用
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(format!(r#"{{"password":"{FAKE_PW}"}}"#)),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let old_session = serde_json::from_str::<serde_json::Value>(&b).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();
        let (s, _) = send(
            app.clone(),
            "GET",
            "/v1/admin/providers",
            None,
            Some(&old_session),
        )
        .await;
        assert_eq!(s, 200);

        // 已有密码时带新密码 = 轮换语义：翻 cloud + 换密 + 旧会话全废
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"cloud","admin_password":"pw-test-rotate-2"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        assert_eq!(
            state.auth.session_generation.load(Ordering::SeqCst),
            1,
            "轮换代际 +1"
        );
        assert!(state.auth.sessions.read().unwrap().is_empty(), "旧会话清空");

        // 旧会话 401、旧密码 401、新密码 200
        let (s, b) = send(
            app.clone(),
            "GET",
            "/v1/admin/providers",
            None,
            Some(&old_session),
        )
        .await;
        assert_eq!(s, 401, "轮换后旧会话必须 401: {b}");
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(format!(r#"{{"password":"{FAKE_PW}"}}"#)),
            None,
        )
        .await;
        assert_eq!(s, 401, "旧密码必须 401: {b}");
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(r#"{"password":"pw-test-rotate-2"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "新密码可登录: {b}");

        // 文件只留新密码
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("pw-test-rotate-2"), "{on_disk}");
        assert!(!on_disk.contains(FAKE_PW), "旧密码被覆盖: {on_disk}");
        cleanup(&path);
    }

    #[tokio::test]
    async fn mode_hot_switch_flips_auth_immediately_and_persists() {
        let content = format!(
            r#"
mode = "local"
auth_tokens = ["{FAKE_TOK}"]
admin_password = "{FAKE_PW}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
        );
        let (app, _state, path) = app_state_of(&content);

        // 切前：无 token 200（local）
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, None).await;
        assert_eq!(s, 200, "切前 local 免 token: {b}");

        // local → cloud（local 态 loopback 可直达 mode 端点，无需会话）
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"cloud"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["mode"], "cloud");
        assert_eq!(v["persisted"], true);
        assert!(std::fs::read_to_string(&path).unwrap().contains("mode = \"cloud\""));

        // 同请求即时翻转：无 token → 401；带 token → 200
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, None).await;
        assert_eq!(s, 401, "切后同请求立即 401: {b}");
        let (s, b) = send(app.clone(), "GET", "/v1/models", None, Some(FAKE_TOK)).await;
        assert_eq!(s, 200, "{b}");

        // cloud 态 mode 端点需会话（防匿名拆锁）：无会话 → 401
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"local"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 401, "cloud 下翻转必须会话: {b}");

        // login 拿会话 → 切回 local
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(format!(r#"{{"password":"{FAKE_PW}"}}"#)),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let session = serde_json::from_str::<serde_json::Value>(&b).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"local"}"#.into()),
            Some(&session),
        )
        .await;
        assert_eq!(s, 200, "{b}");
        assert!(std::fs::read_to_string(&path).unwrap().contains("mode = \"local\""));

        // 切回：无 token 又 200（铁证链闭环）
        let (s, b) = send(app, "GET", "/v1/models", None, None).await;
        assert_eq!(s, 200, "切回 local 恢复免 token: {b}");
        cleanup(&path);
    }

    #[tokio::test]
    async fn env_override_flags_on_mode_and_password_responses() {
        let content = format!(
            r#"
mode = "local"
admin_password = "{FAKE_PW}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
        );
        let (app, state, path) = app_state_of(&content);

        // 全程保持 local（幂等 PUT mode local 也走完整响应管线；避免中途进
        // cloud 撞 admin 会话门 —— 那条链在 mode_hot_switch_* 专测覆盖）。
        // 默认（env 未设，原子位 false）→ 警示 false
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"local"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["env_override_active"], false, "{b}");

        // 拨位模拟「JEV_SWITCH_MODE 活跃」→ 下一次 mode 写响应警示 true
        state
            .auth
            .env_mode_override
            .store(true, Ordering::SeqCst);
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"local"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["env_override_active"], true, "env mode 活跃须警示: {b}");
        // 复位，隔离下一断言只看密码位
        state
            .auth
            .env_mode_override
            .store(false, Ordering::SeqCst);

        // 密码端点：拨 env_password_override → 响应警示 true
        state
            .auth
            .env_password_override
            .store(true, Ordering::SeqCst);
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/password",
            Some(r#"{"password":"pw-test-env-flag-3"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["updated"], true);
        assert_eq!(v["env_override_active"], true, "env 密码活跃须警示: {b}");
        assert!(!b.contains("pw-test-env-flag-3"), "不得回显密码: {b}");

        // mode 端点带密码写入时的联动警示（password_written && env 密码活跃）
        let (s, b) = send(
            app,
            "PUT",
            "/v1/admin/mode",
            Some(r#"{"mode":"local","admin_password":"pw-test-env-flag-4"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["env_override_active"], true, "写了密码且 env 活跃: {b}");
        cleanup(&path);
    }

    #[tokio::test]
    async fn put_password_oneshot_cloud_loopback_allowed_and_kills_sessions() {
        let content = format!(
            r#"
mode = "cloud"
auth_tokens = ["{FAKE_TOK}"]
admin_password = "{FAKE_PW}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:1/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"#
        );
        let (app, state, path) = app_state_of(&content);

        // 旧会话
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(format!(r#"{{"password":"{FAKE_PW}"}}"#)),
            None,
        )
        .await;
        assert_eq!(s, 200, "{b}");
        let old_session = serde_json::from_str::<serde_json::Value>(&b).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();

        // oneshot = None peer = loopback 信任 → cloud 无会话也允许改密（忘密恢复语义）
        let (s, b) = send(
            app.clone(),
            "PUT",
            "/v1/admin/password",
            Some(r#"{"password":"pw-test-loopback-5"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "loopback（None 信任）改密放行: {b}");

        // 旧会话作废、旧密码失效、新密码可登录、文件已写
        let (s, b) = send(
            app.clone(),
            "GET",
            "/v1/admin/providers",
            None,
            Some(&old_session),
        )
        .await;
        assert_eq!(s, 401, "改密后旧 session 401: {b}");
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(format!(r#"{{"password":"{FAKE_PW}"}}"#)),
            None,
        )
        .await;
        assert_eq!(s, 401, "旧密码 401: {b}");
        let (s, b) = send(
            app.clone(),
            "POST",
            "/v1/admin/login",
            Some(r#"{"password":"pw-test-loopback-5"}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 200, "新密码登录: {b}");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("pw-test-loopback-5"), "{on_disk}");
        assert_eq!(
            state.auth.session_generation.load(Ordering::SeqCst),
            1,
            "改密代际 +1"
        );

        // 空密码 → 400
        let (s, b) = send(
            app,
            "PUT",
            "/v1/admin/password",
            Some(r#"{"password":"  "}"#.into()),
            None,
        )
        .await;
        assert_eq!(s, 400, "{b}");
        cleanup(&path);
    }
}
