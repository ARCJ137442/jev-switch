//! Admin API（A7 · contracts/05 §1/§2 + contracts/04 防偷红线）。
//!
//! 端点：
//! - `GET  /v1/admin/providers` → `{providers:[{id,kind,base,enabled,api_key_masked,api_key_set}]}`
//!   **只出掩码**（`sk-****a1b2` 形态），无任何读回明文的字段/路径（红线 2/3）
//! - `PUT  /v1/admin/providers` → 整表替换 + 落盘（0600）；响应回 masked（红线 7）。
//!   `api_key` 省略 = 保留原 key；显式空串 = 清除（回退 `api_key_env`）
//! - `GET  /v1/admin/routes` → `{routes:[RouteEdge]}`（运行时边表 —— 含旧 `[router]`
//!   合并结果，config 载入边集合作为真值）
//! - `PUT  /v1/admin/routes` → 整表替换：字段校验 → **检环 400**（文案含 环/cycle）
//!   → right 引用校验 → 落盘 → `Registry::replace_edges` 热替换（无重启）
//! - `POST /v1/admin/providers/{id}/probe` → `{ok,latency_ms,status,error}`
//!
//! 防偷（contracts/04 §2）：
//! - 响应 DTO [`ProviderView`] 只有 `api_key_masked` / `api_key_set` —— 序列化面
//!   上不存在 `api_key` 字段（单测①⑦双重把守）
//! - 所有错误体 / 探测错误串过 `jev_core::redact`（已知明文 + `sk-` 通用 + Bearer）
//! - 落盘后 `enforce_config_perms`（0600；Windows 降级 warning —— 用户已裁决）
//!
//! 落盘策略（Q5=a 读改写同一文件，其余段不动 —— toml::Value 往返）：
//! - providers PUT：仅替换 `providers` 表；`routes`/`router` 及其它段原样保留
//! - routes PUT：写 `[[routes]]` 并**移除整个旧 `[router]` 表** —— 整表替换语义下
//!   旧扁平映射全部视为已被新表覆盖（payload 若来自 GET 即合并真值；残留任一条
//!   都会在下次加载时重复合并出多余边 / 复活已删边）。**注释不随 toml 往返保留**
//!   （toml crate 不保注释 —— A7 报告备案项）。

use crate::config::{enforce_config_perms, enforce_dir_perms, Config, ProviderConfig};
use crate::{error_response, known_keys_snapshot, AppState};
use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jev_core::{
    redact::{mask_key, redact},
    router::{check_acyclic, MatchMode, RouterError},
};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

/* ══════════════════════════════════════════════════════════════════
   DTO（ts-rs 导出 → ui/src/generated/；RouteEdge 来自 jev-core 一份真值）
   ══════════════════════════════════════════════════════════════════ */

/// GET/PUT 响应视图 —— **字段集即红线 2 字面**：无 `api_key`。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderView {
    pub id: String,
    pub kind: String,
    pub base: String,
    pub enabled: bool,
    /// 掩码形态 `sk-****a1b2`；无有效 key 时为 `""`（配 `api_key_set=false` 看）。
    pub api_key_masked: String,
    pub api_key_set: bool,
}

/// `GET /v1/admin/providers` / `PUT` 响应体。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProvidersDoc {
    pub providers: Vec<ProviderView>,
}

/// `PUT /v1/admin/providers` 单项入参（**仅写入用，永不作响应**）。
/// 不 derive `Serialize` —— 从结构上杜绝「明文 key 被序列化出去」的路径。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderInput {
    pub id: String,
    pub kind: String,
    pub base: String,
    pub enabled: bool,
    /// 新明文 key；**省略 = 保留原 key**；`""` = 清除（回退 `api_key_env`）。
    #[serde(default)]
    pub api_key: Option<String>,
    /// env 名（可选；省略 = 保留已有）。
    #[serde(default)]
    pub api_key_env: Option<String>,
}

/// `PUT /v1/admin/providers` 请求体。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutProvidersBody {
    pub providers: Vec<ProviderInput>,
}

/// `GET/PUT /v1/admin/routes` 共用体（`RouteEdge` = jev-core 导出，一份真值）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct RoutesDoc {
    pub routes: Vec<jev_core::router::RouteEdge>,
}

/// `POST /v1/admin/providers/{id}/probe` 响应（contracts/05 §2 字面四键）。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProbeResult {
    pub ok: bool,
    /// 探测耗时（毫秒）。（ts-rs：u64 默认 → bigint，wire 是 JSON number —— 覆盖对齐运行时）
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub latency_ms: u64,
    /// 上游 HTTP 状态；`0` = 无 HTTP 响应（连接/超时层失败）。
    pub status: u16,
    pub error: Option<String>,
}

/* ══════════════════════════════════════════════════════════════════
   内部工具
   ══════════════════════════════════════════════════════════════════ */

/// 无 state 上下文时的兜底错误体（空已知集 = 只跑通用 redact 模式）。
fn err_plain(status: StatusCode, msg: impl Into<String>) -> Response {
    error_response(status, msg, None, false, &[])
}

fn err(state: &AppState, status: StatusCode, msg: impl Into<String>) -> Response {
    let keys = known_keys_snapshot(state);
    error_response(status, msg, None, false, &keys)
}

fn load_config(state: &AppState) -> Result<Config, Response> {
    Config::load(&state.config_path)
        .map_err(|e| err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config load failed: {e}")))
}

/// 刷新已知密钥集（GET/PUT providers 后调用 —— 外部手改的 key 也纳入 redact）。
fn refresh_known_keys(state: &AppState, cfg: &Config) {
    let mut guard = state.known_keys.write().expect("known_keys lock");
    for id in cfg.providers.keys() {
        if let Some(k) = cfg.effective_api_key(id) {
            if !k.is_empty() && !guard.contains(&k) {
                guard.push(k);
            }
        }
    }
}

/// 构造掩码视图（读路径唯一出口 —— 之后没有任何代码能拿到明文）。
fn provider_view(id: &str, p: &ProviderConfig, keys: &[String]) -> ProviderView {
    let (api_key_masked, api_key_set) = match p.api_key.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => (mask_key(k), true),
        None => match p.api_key_env.as_deref().and_then(|v| std::env::var(v).ok()) {
            Some(v) if !v.is_empty() => (mask_key(&v), true),
            _ => (String::new(), false),
        },
    };
    ProviderView {
        id: id.to_string(),
        kind: p.kind.clone(),
        // 防御性再过一遍 redact（正常 URL 不含 key；防 base 被人为贴 key 的边角）
        base: redact(&p.base, keys),
        enabled: p.enabled,
        api_key_masked,
        api_key_set,
    }
}

fn providers_doc(cfg: &Config, keys: &[String]) -> ProvidersDoc {
    let mut ids: Vec<&String> = cfg.providers.keys().collect();
    ids.sort();
    ProvidersDoc {
        providers: ids
            .iter()
            .map(|id| provider_view(id, &cfg.providers[*id], keys))
            .collect(),
    }
}

/// 读配置文件为 `toml::Value`（读改写用 —— 其余段原样保真；注释不保留）。
fn read_value(path: &Path) -> Result<toml::Value, Response> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config read failed: {e}"))
    })?;
    raw.parse::<toml::Value>().map_err(|e| {
        err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config parse failed: {e}"))
    })
}

/// 落盘 + 权限收紧（0600 文件；0700 仅限默认配置目录 `~/.jev-switch` ——
/// `JEV_SWITCH_CONFIG` 可能指向仓库内示例，不乱 chmod 其父目录）。
fn write_value(path: &Path, value: &toml::Value) -> Result<(), Response> {
    let s = toml::to_string(value).map_err(|e| {
        err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config encode failed: {e}"))
    })?;
    std::fs::write(path, s).map_err(|e| {
        err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config write failed: {e}"))
    })?;
    enforce_config_perms(path);
    if let Some(parent) = path.parent() {
        if parent.ends_with(".jev-switch") {
            enforce_dir_perms(parent);
        }
    }
    Ok(())
}

/* ══════════════════════════════════════════════════════════════════
   GET / PUT · providers
   ══════════════════════════════════════════════════════════════════ */

/// contracts/05 §2：只回 masked —— 序列化面无 `api_key`（红线 2/3）。
pub async fn get_providers(State(state): State<AppState>) -> Response {
    let cfg = match load_config(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    refresh_known_keys(&state, &cfg);
    let keys = known_keys_snapshot(&state);
    Json(providers_doc(&cfg, &keys)).into_response()
}

/// 整表替换写入 + 落盘（0600）；响应 masked（红线 7 —— 不 echo 明文）。
///
/// 逐项语义（H3 交接备注①）：`api_key` **省略 = 保留原 key**、`""` = 清除、
/// 非空 = 替换；`api_key_env` 同理（省略保留）。入参未列出的 provider 被移除。
pub async fn put_providers(State(state): State<AppState>, body: Bytes) -> Response {
    let input: PutProvidersBody = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };

    // 字段校验
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for p in &input.providers {
        if p.id.trim().is_empty() {
            return err(&state, StatusCode::BAD_REQUEST, "provider id 不得为空");
        }
        if p.kind.trim().is_empty() {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("provider '{}' kind 不得为空", p.id),
            );
        }
        if p.base.trim().is_empty() {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("provider '{}' base 不得为空", p.id),
            );
        }
        if !seen.insert(p.id.clone()) {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("provider id 重复: {}", p.id),
            );
        }
    }

    let existing = match load_config(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };

    // 组装新 providers 表（其余段不动）
    let mut prov_table = toml::Table::new();
    for p in &input.providers {
        let old = existing.providers.get(&p.id);
        let api_key: Option<String> = match &p.api_key {
            Some(s) if s.is_empty() => None,           // 显式空 = 清除
            Some(s) => Some(s.clone()),                // 新值
            None => old.and_then(|o| o.api_key.clone()), // 省略 = 保留原 key
        };
        let api_key_env: Option<String> = p
            .api_key_env
            .clone()
            .or_else(|| old.and_then(|o| o.api_key_env.clone()));

        let mut t = toml::Table::new();
        t.insert("kind".into(), toml::Value::String(p.kind.clone()));
        t.insert("base".into(), toml::Value::String(p.base.clone()));
        t.insert("enabled".into(), toml::Value::Boolean(p.enabled));
        if let Some(k) = api_key {
            t.insert("api_key".into(), toml::Value::String(k));
        }
        if let Some(e) = api_key_env {
            t.insert("api_key_env".into(), toml::Value::String(e));
        }
        prov_table.insert(p.id.clone(), toml::Value::Table(t));
    }

    let mut value = match read_value(&state.config_path) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match value.as_table_mut() {
        Some(table) => {
            table.insert("providers".into(), toml::Value::Table(prov_table));
        }
        None => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "config root is not a table",
            )
        }
    }
    if let Err(r) = write_value(&state.config_path, &value) {
        return r;
    }

    // 回读落盘结果构造响应（掩码视图）+ 刷新 redact 密钥集
    match load_config(&state) {
        Ok(cfg) => {
            refresh_known_keys(&state, &cfg);
            let keys = known_keys_snapshot(&state);
            Json(providers_doc(&cfg, &keys)).into_response()
        }
        Err(r) => r,
    }
}

/* ══════════════════════════════════════════════════════════════════
   GET / PUT · routes
   ══════════════════════════════════════════════════════════════════ */

/// 运行时边表（含旧 `[router]` 合并结果 —— config 载入边集合作为真值）。
pub async fn get_routes(State(state): State<AppState>) -> Json<RoutesDoc> {
    Json(RoutesDoc {
        routes: state.registry.router().edges(),
    })
}

/// 整表替换（contracts/05 §2）：校验 → 落盘 → 热替换（无重启）。
///
/// 校验顺序（任务书字面）：字段 → **检环 400**（文案含 环/cycle）→ right 引用。
pub async fn put_routes(State(state): State<AppState>, body: Bytes) -> Response {
    let doc: RoutesDoc = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };

    // 1. 字段校验（serde 已拦类型/枚举值；这里补非空）
    for e in &doc.routes {
        if e.left.trim().is_empty() || e.right.trim().is_empty() {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                "route field invalid: left/right 不得为空",
            );
        }
    }

    // 2. 检环（DAG 约束）→ 400，文案含「环」与 cycle
    if let Err(e) = check_acyclic(&doc.routes) {
        let msg = match &e {
            RouterError::Cycle(path) => format!("路由配置存在环 (cycle): {path}"),
            other => other.to_string(),
        };
        return err(&state, StatusCode::BAD_REQUEST, msg);
    }

    // 3. right 引用：已注册 provider（配置 providers 表）∪ 新表 exact 边的 left（别名节点）。
    //    注：`Config::load` 本体不拒悬空 right（仅检环）—— PUT 侧按任务书字面更严，
    //    备案见 A7 报告。
    let existing = match load_config(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let alias_lefts: BTreeSet<&str> = doc
        .routes
        .iter()
        .filter(|e| e.r#match == MatchMode::Exact)
        .map(|e| e.left.as_str())
        .collect();
    for e in &doc.routes {
        if existing.providers.contains_key(&e.right) || alias_lefts.contains(e.right.as_str()) {
            continue;
        }
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            format!(
                "route invalid: 边 '{}' 的 right '{}' 既不是已注册 provider，也不是新表中的别名节点",
                e.left, e.right
            ),
        );
    }

    // 4. 落盘：写 [[routes]] + 移除整个旧 [router]（整表替换语义，见模块文档注释）
    let mut value = match read_value(&state.config_path) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut routes_arr = toml::value::Array::new();
    for e in &doc.routes {
        match toml::Value::try_from(e) {
            Ok(v) => routes_arr.push(v),
            Err(e) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("route encode: {e}"),
                )
            }
        }
    }
    match value.as_table_mut() {
        Some(table) => {
            table.remove("router"); // 以新表为准：旧扁平映射全部视为被覆盖
            table.insert("routes".into(), toml::Value::Array(routes_arr));
        }
        None => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "config root is not a table",
            )
        }
    }
    if let Err(r) = write_value(&state.config_path, &value) {
        return r;
    }

    // 5. 运行时热替换（边表整换；上游注册不动；sticky 记忆随之清空）
    state.registry.replace_edges(doc.routes.clone());

    Json(doc).into_response()
}

/* ══════════════════════════════════════════════════════════════════
   POST · providers/{id}/probe
   ══════════════════════════════════════════════════════════════════ */

/// 轻量真实探测 —— **方案：HTTP 连通探测（GET base，无 auth、无 body、5s 超时）**。
///
/// 取舍（A7 报告备案）：
/// - 最小 `evaluate` 对 Vercel 有**真实计费**风险 → 不做；GET 不触发评估、零计费面
/// - 任意 HTTP 响应 = 连通 → `ok=true`，`status` 透出真实码（200/404/405/401 均代表
///   服务可达）；网络/超时层失败 → `ok=false, status=0, error=redact(原因)`
/// - 鉴权探测（带 Bearer 区分 401）留待后续：需按厂商方言构造免计费请求
pub async fn probe_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let cfg = match load_config(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(p) = cfg.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };

    let keys = known_keys_snapshot(&state);
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("probe client: {e}"),
            )
        }
    };

    let t0 = Instant::now();
    let result = match client.get(&p.base).send().await {
        Ok(resp) => ProbeResult {
            ok: true, // 拿到 HTTP 响应 = 连通
            latency_ms: t0.elapsed().as_millis() as u64,
            status: resp.status().as_u16(),
            error: None,
        },
        Err(e) => ProbeResult {
            ok: false,
            latency_ms: t0.elapsed().as_millis() as u64,
            status: 0,
            error: Some(redact(&e.to_string(), &keys)),
        },
    };
    Json(result).into_response()
}

/* ══════════════════════════════════════════════════════════════════
   测试（A7 必须单测 ①③④⑤⑥⑦；② redact 在 jev-core::redact）
   ══════════════════════════════════════════════════════════════════ */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_app, build_state};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// 假 key（测试专用假值 —— 真实密钥永不进测试/提交）。
    const FAKE_KEY: &str = "sk-test1234abcd";

    /// 每测独立临时配置（不碰 `JEV_SWITCH_CONFIG` 环境变量 —— 进程级会打架，
    /// 测试一律显式传 path）。
    fn temp_config(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jev-admin-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("providers.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    fn app_at(path: std::path::PathBuf) -> (axum::Router, AppState) {
        let cfg = Config::load(&path).expect("load temp config");
        let state = build_state(cfg, path);
        let app = build_app(state.clone());
        (app, state)
    }

    async fn send(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> (u16, String) {
        let builder = Request::builder().method(method).uri(uri).header("content-type", "application/json");
        let req = match body {
            Some(b) => builder.body(Body::from(b)).unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = app.oneshot(req).await.expect("oneshot");
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    const CFG_WITH_KEY: &str = r#"
[providers.vercel]
kind = "vercel"
base = "https://example.invalid/v4/eval"
api_key = "sk-test1234abcd"
enabled = true

[providers.laya]
kind = "laya"
base = "http://127.0.0.1:18765/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"

[[routes]]
left = "jev"
right = "vercel"
priority = 10
"#;

    /* ── 单测①：GET providers 全响应字符串不含配置明文 key ──────── */

    #[tokio::test]
    async fn get_providers_never_leaks_plaintext_key() {
        let path = temp_config("get-mask", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        let (status, body) = send(app, "GET", "/v1/admin/providers", None).await;
        assert_eq!(status, 200, "body={body}");

        // 全响应字符串断言：明文不出现、掩码形态出现、api_key 字段名不出现
        assert!(!body.contains(FAKE_KEY), "响应泄露明文 key: {body}");
        assert!(body.contains("sk-****abcd"), "应含掩码: {body}");
        assert!(body.contains(r#""api_key_masked""#), "{body}");
        assert!(body.contains(r#""api_key_set":true"#), "{body}");
        // 裸 api_key 字段（非 masked/set）不得出现
        assert!(!body.contains(r#""api_key":"#), "出现明文字段名: {body}");

        // 结构级：ProviderView 序列化键集 = 6 键且无 api_key
        let doc: serde_json::Value = serde_json::from_str(&body).unwrap();
        let vercel = doc["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "vercel")
            .expect("vercel in list");
        let obj = vercel.as_object().unwrap();
        assert_eq!(obj.len(), 6);
        assert!(obj.get("api_key").is_none(), "键集不得含 api_key");
        // laya 无 key → set=false + 空掩码
        let laya = doc["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "laya")
            .unwrap();
        assert_eq!(laya["api_key_set"], false);
        assert_eq!(laya["api_key_masked"], "");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 单测③：PUT routes 成环 → 400（文案含 环/cycle） ────────── */

    #[tokio::test]
    async fn put_routes_cycle_is_400_with_cycle_message() {
        let path = temp_config("cycle", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        let body = r#"{"routes":[
            {"left":"a","right":"b","priority":1},
            {"left":"b","right":"a","priority":2}
        ]}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/routes", Some(body.into())).await;
        assert_eq!(status, 400, "resp={resp}");
        assert!(resp.contains("环") || resp.contains("cycle"), "文案须含 环/cycle: {resp}");

        // 文件未被污染（检环在落盘前）
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("left = \"a\""), "环配置不得落盘: {on_disk}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 单测④：PUT routes 成功 → 运行时 Router 热更 ────────────── */

    #[tokio::test]
    async fn put_routes_success_hot_replaces_runtime_router() {
        let path = temp_config("hot", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());

        // 热更前：jev 可选路
        let before = state
            .registry
            .router()
            .select("jev", &jev_core::router::RouteCtx::default());
        assert!(before.is_ok(), "热更前 jev 应可选路");

        let body = r#"{"routes":[
            {"left":"new-model","right":"vercel","upstream_model":"typesafe-ai/jev","priority":7}
        ]}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/routes", Some(body.into())).await;
        assert_eq!(status, 200, "resp={resp}");

        // 运行时热更生效：新边命中、旧边消失（无重启）
        let after = state
            .registry
            .router()
            .select("new-model", &jev_core::router::RouteCtx::default())
            .expect("新边应命中");
        assert_eq!(after[0].upstream_id, "vercel");
        assert_eq!(after[0].priority, 7);
        let gone = state
            .registry
            .router()
            .select("jev", &jev_core::router::RouteCtx::default());
        assert!(gone.is_err(), "旧 jev 边应已消失");

        // GET 回读 = 新表（build_app 幂等 —— oneshot 消耗 app，用 state 重建）
        let (gs, grest) = send(build_app(state.clone()), "GET", "/v1/admin/routes", None).await;
        assert_eq!(gs, 200, "{grest}");
        assert!(grest.contains("new-model"), "{grest}");
        assert!(!grest.contains(r#""left":"jev""#), "{grest}");

        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("new-model"), "{on_disk}");
        assert!(!on_disk.contains("[router]"), "旧 [router] 应被整表替换移除: {on_disk}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 单测⑥：probe 形状（四键冻结） ─────────────────────────── */

    #[tokio::test]
    async fn probe_result_shape_frozen_and_unreachable_reports_false() {
        // base 指向必然拒绝的 loopback 端口 → 连通失败分支
        let cfg = r#"
[providers.dead]
kind = "vercel"
base = "http://127.0.0.1:9/x"
enabled = true
"#;
        let path = temp_config("probe", cfg);
        let (app, _state) = app_at(path.clone());

        let (status, body) = send(
            app,
            "POST",
            "/v1/admin/providers/dead/probe",
            Some("{}".into()),
        )
        .await;
        assert_eq!(status, 200, "probe 端点本身 200（ok 表达连通结果）: {body}");

        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let obj = v.as_object().expect("probe 是对象");
        // 四键形状冻结（contracts/05 §2）
        assert_eq!(obj.len(), 4, "{body}");
        assert!(obj.contains_key("ok"));
        assert!(obj.contains_key("latency_ms"));
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("error"));
        assert_eq!(v["ok"], false, "拒绝连接 → ok=false");
        assert_eq!(v["status"], 0, "无 HTTP 响应 → status=0");
        assert!(v["error"].is_string(), "error 应给出原因: {body}");
        assert!(v["latency_ms"].is_number());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn probe_reachable_wiremock_ok_true() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let cfg = format!(
            r#"
[providers.mock]
kind = "laya"
base = "{}"
enabled = true
"#,
            server.uri()
        );
        let path = temp_config("probe-ok", &cfg);
        let (app, _state) = app_at(path.clone());

        let (status, body) = send(
            app,
            "POST",
            "/v1/admin/providers/mock/probe",
            Some("{}".into()),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["ok"], true, "{body}");
        assert_eq!(v["status"], 204, "{body}");
        assert_eq!(v["error"], serde_json::Value::Null, "{body}");
        assert!(v["latency_ms"].is_number());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn probe_unknown_provider_is_404() {
        let path = temp_config("probe-404", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());
        let (status, body) = send(
            app,
            "POST",
            "/v1/admin/providers/ghost/probe",
            Some("{}".into()),
        )
        .await;
        assert_eq!(status, 404, "{body}");
        assert!(body.contains("ghost"), "{body}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 单测⑦：源码 grep —— 无任何 Serialize 面声明 api_key 字段 ── */

    #[test]
    fn no_serialized_struct_declares_plaintext_api_key_field() {
        // 扫描 daemon 全部 src：凡 derive 含 Serialize 的 struct，字段名不得是
        // 裸 `api_key`（ProviderInput 只 Deserialize —— 写入面，允许）。
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut scanned = 0usize;
        for entry in std::fs::read_dir(&src_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = src.lines().collect();
            let mut i = 0;
            while i < lines.len() {
                if lines[i].trim_start().starts_with("#[derive(") {
                    let mut derive_text = String::new();
                    let mut j = i;
                    while j < lines.len() {
                        derive_text.push_str(lines[j]);
                        if lines[j].contains(']') {
                            break;
                        }
                        j += 1;
                    }
                    if derive_text.contains("Serialize") {
                        // derive 之后找 struct Name { … }
                        let mut k = j + 1;
                        while k < lines.len() && !lines[k].contains("struct ") {
                            k += 1;
                        }
                        if k < lines.len() {
                            let name_line = &lines[k];
                            if let Some(brace) = name_line.find('{') {
                                let name: String = name_line
                                    .split("struct ")
                                    .nth(1)
                                    .unwrap_or("")
                                    .split_whitespace()
                                    .next()
                                    .unwrap_or("?")
                                    .to_string();
                                // 抓字段（这些 DTO 字段行无嵌套花括号）
                                let mut m = k;
                                let mut fields = String::new();
                                while m < lines.len() {
                                    fields.push_str(lines[m]);
                                    if m > k && lines[m].trim_start().starts_with('}') {
                                        break;
                                    }
                                    if m == k && lines[k][brace + 1..].trim_start().starts_with('}')
                                    {
                                        break;
                                    }
                                    m += 1;
                                }
                                scanned += 1;
                                let has_plain = fields
                                    .lines()
                                    .map(|l| l.trim())
                                    .filter(|l| !l.starts_with("//") && !l.starts_with('#'))
                                    .any(|l| l.starts_with("api_key:") || l.starts_with("pub api_key:"));
                                assert!(
                                    !has_plain,
                                    "Serialize struct `{name}` ({}) 声明了裸 api_key 字段 —— 违反红线 3（无读回明文）",
                                    path.display()
                                );
                            }
                        }
                    }
                    i = j + 1;
                    continue;
                }
                i += 1;
            }
        }
        assert!(scanned >= 8, "至少应扫到 8 个 Serialize struct，实际 {scanned}");
    }

    /* ── PUT providers：省略 api_key = 保留原 key（H3 备注①） ──── */

    #[tokio::test]
    async fn put_providers_omitted_api_key_preserves_existing() {
        let path = temp_config("put-keep", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        // 只翻 enabled、不带 api_key → 原 key 保留
        let body = r#"{"providers":[
            {"id":"vercel","kind":"vercel","base":"https://example.invalid/v4/eval","enabled":false},
            {"id":"laya","kind":"laya","base":"http://127.0.0.1:18765/v1/systemone","enabled":true}
        ]}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/providers", Some(body.into())).await;
        assert_eq!(status, 200, "{resp}");
        // 响应 masked、enabled 已翻转
        assert!(!resp.contains(FAKE_KEY), "PUT 响应不得 echo 明文: {resp}");
        assert!(resp.contains("sk-****abcd"), "{resp}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        let vercel = v["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "vercel")
            .unwrap();
        assert_eq!(vercel["enabled"], false);
        assert_eq!(vercel["api_key_set"], true);

        // 落盘核对：api_key 原文保留、enabled 翻转
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains(FAKE_KEY), "省略 api_key 应保留原 key: {on_disk}");
        assert!(on_disk.contains("enabled = false"), "{on_disk}");
        // 0600（Unix assert；Windows 分支 warning 的 cfg 测在 config.rs）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "PUT 写回后必须 0600");
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn put_providers_rejects_duplicate_and_empty_fields() {
        let path = temp_config("put-bad", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());

        let dup = r#"{"providers":[
            {"id":"a","kind":"k","base":"b","enabled":true},
            {"id":"a","kind":"k","base":"b","enabled":true}
        ]}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/providers", Some(dup.into())).await;
        assert_eq!(status, 400, "{resp}");

        let empty_id = r#"{"providers":[{"id":"","kind":"k","base":"b","enabled":true}]}"#;
        let (status, resp) = send(
            build_app(state.clone()),
            "PUT",
            "/v1/admin/providers",
            Some(empty_id.into()),
        )
        .await;
        assert_eq!(status, 400, "{resp}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── PUT routes right 引用校验 ──────────────────────────────── */

    #[tokio::test]
    async fn put_routes_rejects_unknown_right_node() {
        let path = temp_config("right", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        // right 既非 provider（vercel/laya）也非新表 exact left → 400
        let body = r#"{"routes":[{"left":"jev","right":"ghost","priority":1}]}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/routes", Some(body.into())).await;
        assert_eq!(status, 400, "{resp}");
        assert!(resp.contains("ghost"), "{resp}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 别名 right 合法（新表 exact left 即别名节点） ──────────── */

    #[tokio::test]
    async fn put_routes_accepts_alias_right_within_payload() {
        let path = temp_config("alias", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        let body = r#"{"routes":[
            {"left":"jev","right":"jev-fast","priority":5},
            {"left":"jev-fast","right":"vercel","priority":10}
        ]}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/routes", Some(body.into())).await;
        assert_eq!(status, 200, "{resp}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
