//! #43 双态鉴权（docs/12 §一/§二.6/§四线1 —— 裁决已锁，不重新设计）。
//!
//! - **local（默认，现状回归）**：两道中间件直接放行 —— 端点行为与接入前完全一致。
//! - **cloud**：
//!   - `/v1/systemone`、`/v1/models` → `Authorization: Bearer <调用 token>`
//!     （token 来自配置 `auth_tokens` 两种写法 / env `JEV_AUTH_TOKENS`；
//!     单账号语义，**不做 token 管理体系**）。缺失/错值 → **401** 统一错误体
//!     `{error, upstream:null, retryable:false}`（经 redact，contracts/05 §3 形状）。
//!   - `/health` **放行**（容器/探针探活，任务书字面）。
//!   - `/v1/admin/*` → 独立管理密码：`POST /v1/admin/login {password}` 换**短时会话
//!     token**（内存态、不落盘、过期即废）→ 后续 admin 请求带
//!     `Authorization: Bearer <会话 token>`。密码来源：配置 `admin_password`
//!     （明文 toml —— Q4=b 哲学）/ env `JEV_ADMIN_PASSWORD` 优先。
//!
//! **login 形态选择（任务书二选一 → 报告说明）**：取「登录换会话 token」标准形态，
//! 而非每请求 `X-Admin-Password` 头 —— 理由：① 密码只在网络出现一次，之后只有
//! 短时 token 参与传输，缩小密码暴露面；② 会话可过期/失效（改密码重启即全废），
//! 头形态每请求都是有效凭据；③ 浏览器侧只存 token 不存密码。
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
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// 运行模式（启动时由 `Config::effective_mode` 解析一次；mid-life 不变）。
    pub mode: crate::config::RunMode,
    /// cloud `/v1` 调用 token 列表（local 态忽略）。
    pub auth_tokens: Vec<String>,
    /// cloud admin 登录密码（明文 —— 仅存在内存；无 = admin 登录 fail-closed）。
    pub admin_password: Option<String>,
    /// admin 会话：token → **过期时刻**（内存态，不落盘；重启全废）。
    /// 存 `expires_at` 而非签发时刻 —— prune 判断不依赖 `now - created` 的
    /// 时钟回退语义（Instant 单调，但存到期点让测试可注入 1s 前的过期戳，
    /// 无「机器 uptime < TTL」假设）。
    pub sessions: RwLock<HashMap<String, Instant>>,
}

impl Default for AuthState {
    /// local 默认（缺省零鉴权 —— 现状回归）。
    fn default() -> Self {
        AuthState {
            mode: crate::config::RunMode::Local,
            auth_tokens: Vec::new(),
            admin_password: None,
            sessions: RwLock::new(HashMap::new()),
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
            mode,
            auth_tokens,
            admin_password,
            sessions: RwLock::new(HashMap::new()),
        }
    }
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
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
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

/// 401 统一错误体（contracts/05 §3 形状；经 redact —— 红线 4）。
fn unauthorized(state: &AppState, msg: &str) -> Response {
    let keys = crate::known_keys_snapshot(state);
    error_response(StatusCode::UNAUTHORIZED, msg, None, false, &keys)
}

/* ══════════════════════════════════════════════════════════════════
   中间件（local 直通 / cloud 校验）
   ══════════════════════════════════════════════════════════════════ */

/// `/v1/systemone`、`/v1/models` 的调用 token 门。
pub async fn require_call_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if state.auth.mode == crate::config::RunMode::Local {
        return next.run(req).await; // 现状回归：零校验
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

/// `/v1/admin/*` 的会话门（login 端点本身不走此门 —— 在 build_app 挂在门外）。
pub async fn require_admin_session(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if state.auth.mode == crate::config::RunMode::Local {
        return next.run(req).await; // 现状回归：零校验
    }
    let Some(tok) = bearer_token(req.headers()) else {
        return unauthorized(&state, "admin session required (POST /v1/admin/login)");
    };
    // 锁作用域块内完成 prune + 查验 —— `RwLockWriteGuard` 是 !Send，
    // 绝不跨越 `next.run(...).await`（否则整个 middleware future 非 Send，
    // `from_fn_with_state` 的 Service 约束不满足）。
    let valid = {
        let mut sessions = state
            .auth
            .sessions
            .write()
            .expect("admin sessions lock");
        prune_expired(&mut sessions);
        sessions.contains_key(tok)
    };
    if valid {
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

    let Some(configured) = state.auth.admin_password.as_deref() else {
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
}
