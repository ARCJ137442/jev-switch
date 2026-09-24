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
//! - `PUT  /v1/admin/mode` → **mode 热切**（不重启）：可选 `admin_password`
//!   激活/轮换 → 写 toml `mode` → 写 `AuthState.mode` RwLock → 非显式 bind 时
//!   联动 ListenSupervisor 热 Rebind 到成对默认（见 [`crate::listen`]）
//! - `PUT/GET /v1/admin/listen` → **监听热 Rebind**：`{"addr":"ip:port"}` 显式化 /
//!   `{"addr":"auto"}` 恢复成对默认；try-bind 失败保旧（带病不上线）
//! - `PUT  /v1/admin/password` → admin 密码热更（**门外**端点，in-handler 鉴权：
//!   local=loopback；cloud=有效会话 **或** loopback peer —— 服务器 SSH 上机一行
//!   curl 忘密恢复）；一切密码变更统一走 [`set_admin_password`]
//!
//! Phase 4.2: 服务入口配置端点（见 endpoints 子模块）：
//! - `GET  /v1/admin/endpoints` → 获取所有服务入口 + 健康状态 + 调用统计
//! - `POST /v1/admin/endpoints` → 创建服务入口 + 可选路由
//! - `PUT  /v1/admin/endpoints/{id}` → 更新策略/启用状态，支持修改 ID
//! - `DELETE /v1/admin/endpoints/{id}` → 删除入口 + 级联删除路由
//! - `GET  /v1/admin/config/default_strategy` → 获取全局默认策略
//! - `PUT  /v1/admin/config/default_strategy` → 更新全局默认策略
//!
//! 防偷（contracts/04 §2）：
//! - 响应 DTO [`ProviderView`] 只有 `api_key_masked` / `api_key_set` —— 序列化面
//!   上不存在 `api_key` 字段（单测①⑦双重把守）
//! - 所有错误体 / 探测错误串过 `jev_core::redact`（已知明文 + `sk-` 通用 + Bearer）
//! - 落盘后 `enforce_config_perms`（0600；Windows 降级 warning —— 用户已裁决）
//! - **密码永不回传**：mode/password 响应只出布尔警示字段，无 password 键
//!
//! 落盘策略（Q5=a 读改写同一文件，其余段不动 —— toml::Value 往返）：
//! - providers PUT：仅替换 `providers` 表；`routes`/`router` 及其它段原样保留
//! - routes PUT：写 `[[routes]]` 并**移除整个旧 `[router]` 表** —— 整表替换语义下
//!   旧扁平映射全部视为已被新表覆盖（payload 若来自 GET 即合并真值；残留任一条
//!   都会在下次加载时重复合并出多余边 / 复活已删边）。**注释不随 toml 往返保留**
//!   （toml crate 不保注释 —— A7 报告备案项）。
//! - mode/password/listen PUT：写对应单键，其余段原样（同上注释不保）。

pub mod endpoints;

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
use std::sync::atomic::Ordering;
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
pub(crate) fn read_value(path: &Path) -> Result<toml::Value, Response> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config read failed: {e}"))
    })?;
    raw.parse::<toml::Value>().map_err(|e| {
        err_plain(StatusCode::INTERNAL_SERVER_ERROR, format!("config parse failed: {e}"))
    })
}

/// 落盘 + 权限收紧（0600 文件；0700 仅限默认配置目录 `~/.jev-switch` ——
/// `JEV_SWITCH_CONFIG` 可能指向仓库内示例，不乱 chmod 其父目录）。
pub(crate) fn write_value(path: &Path, value: &toml::Value) -> Result<(), Response> {
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
   PUT /v1/admin/mode · PUT|GET /v1/admin/listen · PUT /v1/admin/password
   （mode 热切 / listen 热 Rebind / 密码热更 —— 零进程重启）
   ══════════════════════════════════════════════════════════════════ */

/// `PUT /v1/admin/mode` 请求体。`admin_password` 可选：
/// - cloud 激活且**从未配置过密码** → **必带**（400 指引文案，fail-closed 不破）；
/// - 已有密码且带了 → **轮换语义**（覆盖持久化 + 会话代际作废）。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutModeBody {
    pub mode: String,
    #[serde(default)]
    pub admin_password: Option<String>,
}

/// Rebind 信息（`PUT /mode` 响应成员）：执行了 → `{from,to,ok[,reason]}`；
/// 未执行 → `{"skipped":"explicit bind" | "no listener"}`（untagged 判别）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(untagged)]
pub enum RebindInfo {
    Done {
        from: String,
        to: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Skipped {
        skipped: String,
    },
}

/// `PUT /v1/admin/mode` 响应。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutModeResponse {
    pub mode: crate::config::RunMode,
    pub persisted: bool,
    /// env 覆盖警示：`JEV_SWITCH_MODE` 非空（下次启动覆盖文件 mode），
    /// 或本次写了密码且 `JEV_ADMIN_PASSWORD` 非空（下次启动覆盖回去）。
    pub env_override_active: bool,
    pub rebind: RebindInfo,
}

/// `PUT /v1/admin/listen` 请求体（`"auto"` = 恢复成对默认，按当前 mode）。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutListenBody {
    pub addr: String,
}

/// `PUT /v1/admin/listen` 成功响应。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutListenResponse {
    pub addr: String,
    pub rebound: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `GET /v1/admin/listen` 响应（当前实际监听地址）。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct GetListenResponse {
    pub addr: String,
}

/// 进程启动时刻（`build_state` 初始化一次；`GET /v1/admin/status.uptime_s` 用）。
pub(crate) static PROCESS_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// `GET /v1/admin/status` 响应 —— **7 键冻结**（首页仪表盘数据源，UI 已钉死字段名）。
/// `password_set` 只是布尔；**任何字段都不得携带密码值**（contracts/04 §2）。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct StatusResponse {
    pub mode: crate::config::RunMode,
    /// 实际当前监听地址（Rebind 后实时反映；无 supervisor 的 oneshot 路径回退
    /// 配置解析值）。
    pub bind: String,
    /// 是否显式配置 bind（true 时 mode 翻转不改监听）。
    pub bind_explicit: bool,
    /// env `JEV_SWITCH_MODE` 活跃（下次启动覆盖文件 mode）。
    pub env_override_active: bool,
    /// 管理密码是否已配置（**绝不返回密码值**）。
    pub password_set: bool,
    pub version: String,
    /// 进程启动至今秒数。（ts-rs：u64 默认 bigint，wire 是 JSON number → 覆盖）
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub uptime_s: u64,
}

/// `GET /v1/admin/status` —— 首页仪表盘 / 运维一眼自检（模式、监听、是否设密）。
/// 鉴权走 admin 门（cloud=会话；local=loopback —— 与同集合端点一致，login 除外规则不变）。
pub async fn get_status(State(state): State<AppState>) -> Response {
    let mode = crate::auth::current_mode(&state);
    let (bind, bind_explicit) = match state.listen.get() {
        Some(h) => (h.bound().to_string(), h.is_explicit()),
        // 无 supervisor（oneshot 单测）：回退配置解析值（成对默认 / 显式 bind）
        None => match Config::load(&state.config_path)
            .ok()
            .and_then(|c| c.effective_bind(mode).ok())
        {
            Some((addr, exp)) => (addr.to_string(), exp),
            None => (String::new(), false),
        },
    };
    Json(StatusResponse {
        mode,
        bind,
        bind_explicit,
        env_override_active: state.auth.env_mode_override.load(Ordering::SeqCst),
        password_set: state
            .auth
            .admin_password
            .read()
            .expect("password lock")
            .is_some(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_s: PROCESS_START
            .get_or_init(Instant::now)
            .elapsed()
            .as_secs(),
    })
    .into_response()
}

/// `PUT /v1/admin/password` 请求体。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutPasswordBody {
    pub password: String,
}

/// `PUT /v1/admin/password` 响应（**无 password 键** —— 防偷红线）。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutPasswordResponse {
    pub updated: bool,
    /// env `JEV_ADMIN_PASSWORD` 非空警示（下次启动覆盖回文件值）。
    pub env_override_active: bool,
}

/// **一切密码变更的统一内部入口**（mode 激活 / mode 轮换 / password 端点三路
/// 共用，防语义漂移）：
/// 1. 持久化：toml 写 `admin_password = "…"`（0600，Q4=b 与 `api_key` PUT 同模式）；
/// 2. 立即生效：`AuthState.admin_password` RwLock 写入（登录即用新值）；
/// 3. **会话代际 +1 + `sessions.clear()`** —— 作废一切旧会话；
/// 4. 新密码纳入 redact known_keys 集（contracts/04 §2）。
///
/// 密码值**绝不进日志/tracing/响应**。失败 → 三键错误体 500（调用方不得继续
/// 激活 mode —— fail-closed 优先）。
pub(crate) fn set_admin_password(state: &AppState, new_password: &str) -> Result<(), Response> {
    let mut value = read_value(&state.config_path)?;
    match value.as_table_mut() {
        Some(table) => {
            table.insert(
                "admin_password".into(),
                toml::Value::String(new_password.to_string()),
            );
        }
        None => {
            return Err(err_plain(
                StatusCode::INTERNAL_SERVER_ERROR,
                "config root is not a table",
            ))
        }
    }
    write_value(&state.config_path, &value)?;
    // 运行时生效（读锁先释放再写 —— std RwLock 不可重入）
    *state
        .auth
        .admin_password
        .write()
        .expect("password lock") = Some(new_password.to_string());
    // 会话代际作废：清空 + 计数器 +1（旧 token 即刻 401）
    state
        .auth
        .sessions
        .write()
        .expect("admin sessions lock")
        .clear();
    state.auth.session_generation.fetch_add(1, Ordering::SeqCst);
    // redact 集扩容（旧密码保留也无妨 —— 防旧值仍出现在历史串里）
    {
        let mut keys = state.known_keys.write().expect("known_keys lock");
        if !new_password.is_empty() && !keys.iter().any(|k| k == new_password) {
            keys.push(new_password.to_string());
        }
    }
    tracing::info!("admin password updated (value not logged)");
    Ok(())
}

/// `PUT /v1/admin/mode` —— mode 热切（鉴权策略 + 可选 Rebind + 持久化）。
///
/// 鉴权由 `require_admin_session` 中间件承担（cloud → 有效会话；local → loopback
/// peer —— 显式 0.0.0.0 bind 时非 loopback 已被 403 挡在门外）。
///
/// 执行序（校验先行，fail-closed 不破）：
/// 1. 解析 mode（非法 400）+ 密码字段校验（空串 400）；
/// 2. cloud 激活且从未配置密码且未带 → **400 指引**；
/// 3. 带了密码 → [`set_admin_password`]（持久化+代际作废；失败 500 不激活）；
/// 4. 写 toml `mode` → 500（此时密码已改、mode 未翻 —— 保守安全态）；
/// 5. 写 `AuthState.mode` RwLock → 后续请求立即生效；
/// 6. 非显式 bind → Rebind 成对默认；显式 → `skipped:"explicit bind"`。
pub async fn put_mode(State(state): State<AppState>, body: Bytes) -> Response {
    // 1. 解析
    let req: PutModeBody = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid mode body: {e}")),
    };
    let new_mode = match crate::config::RunMode::parse(&req.mode) {
        Ok(m) => m,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, e.to_string()),
    };
    if let Some(pw) = &req.admin_password {
        if pw.trim().is_empty() {
            return err(&state, StatusCode::BAD_REQUEST, "admin_password must not be empty");
        }
    }

    // 同步运行时密码（Q5 文件真值：外部手改/env 在激活路径拾起）。
    // 注意：判空必须在**独立语句**完成 —— 若写在 `if cond {}` 条件里，read 临时
    // guard 会横跨整个 if 块，块内再 take write → 同线程死锁（RwLock 不可重入）。
    let need_password_sync = state
        .auth
        .admin_password
        .read()
        .expect("password lock")
        .is_none();
    if need_password_sync {
        if let Ok(file_cfg) = Config::load(&state.config_path) {
            if let Some(eff) = file_cfg.effective_admin_password() {
                *state
                    .auth
                    .admin_password
                    .write()
                    .expect("password lock") = Some(eff);
            }
        }
    }
    let has_password = state
        .auth
        .admin_password
        .read()
        .expect("password lock")
        .is_some();

    // 2. cloud 激活密码闸（从未配置 → 必带第一个密码）
    if new_mode == crate::config::RunMode::Cloud && !has_password && req.admin_password.is_none() {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "admin password required to activate cloud: include admin_password in this request, or set JEV_ADMIN_PASSWORD / toml admin_password first",
        );
    }

    // 3. 密码热更（可选 —— 首设或轮换）；失败 500 且不激活
    let mut password_written = false;
    if let Some(pw) = &req.admin_password {
        if let Err(r) = set_admin_password(&state, pw) {
            return r;
        }
        password_written = true;
    }

    // 4. 持久化 mode（Q5 文件真值）
    let mut value = match read_value(&state.config_path) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match value.as_table_mut() {
        Some(table) => {
            table.insert(
                "mode".into(),
                toml::Value::String(new_mode.as_str().to_string()),
            );
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

    // 5. 运行时翻转（后续请求立即生效；在途请求跑完旧策略）
    *state.auth.mode.write().expect("mode lock") = new_mode;
    tracing::info!(mode = new_mode.as_str(), "mode hot-switched");

    // 6. Rebind 联动（非显式 bind 才动监听）
    let rebind = match state.listen.get() {
        None => RebindInfo::Skipped {
            skipped: "no listener".into(),
        },
        Some(h) if h.is_explicit() => RebindInfo::Skipped {
            skipped: "explicit bind".into(),
        },
        Some(h) => {
            let to = h.defaults().for_mode(new_mode);
            match h.rebind(to).await {
                Ok((from, to)) => RebindInfo::Done {
                    from: from.to_string(),
                    to: to.to_string(),
                    ok: true,
                    reason: None,
                },
                Err(e) => RebindInfo::Done {
                    from: h.bound().to_string(),
                    to: to.to_string(),
                    ok: false,
                    reason: Some(e.to_string()),
                },
            }
        }
    };

    let env_override_active = state.auth.env_mode_override.load(Ordering::SeqCst)
        || (password_written && state.auth.env_password_override.load(Ordering::SeqCst));
    Json(PutModeResponse {
        mode: new_mode,
        persisted: true,
        env_override_active,
        rebind,
    })
    .into_response()
}

/// `PUT /v1/admin/listen` —— 监听热 Rebind（try-bind 失败保旧，带病不上线）。
///
/// - `{"addr":"127.0.0.1:2222"}` → Rebind + **写回 toml `bind`**（此后变显式绑定，
///   mode 翻转不再动监听）；
/// - `{"addr":"auto"}` → 恢复成对默认（按**当前** mode）+ **移除 toml `bind` 键**。
///
/// 鉴权同 mode（admin 中间件）。失败：解析 400 / supervisor 缺失 503 /
/// try-bind 失败 500 —— 均三键错误体（contracts/05 §3），旧监听原样保留。
pub async fn put_listen(State(state): State<AppState>, body: Bytes) -> Response {
    let req: PutListenBody = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid listen body: {e}")),
    };
    let Some(handle) = state.listen.get() else {
        return err(&state, StatusCode::SERVICE_UNAVAILABLE, "listen supervisor unavailable");
    };

    // 目标地址：auto → 当前 mode 成对默认；否则解析显式值
    let (target, explicit_after) = if req.addr.trim().eq_ignore_ascii_case("auto") {
        (
            handle.defaults().for_mode(crate::auth::current_mode(&state)),
            false,
        )
    } else {
        match req.addr.trim().parse::<std::net::SocketAddr>() {
            Ok(a) => (a, true),
            Err(_) => {
                return err(
                    &state,
                    StatusCode::BAD_REQUEST,
                    format!("invalid bind addr '{}' (expected ip:port, or \"auto\")", req.addr),
                )
            }
        }
    };

    // Rebind（失败 → 旧监听未动 / 已恢复；不写文件）
    if let Err(e) = handle.rebind(target).await {
        return err(&state, StatusCode::INTERNAL_SERVER_ERROR, format!("rebind failed: {e}"));
    }

    // 持久化 bind 键（显式化 / auto 移除）
    let mut value = match read_value(&state.config_path) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match value.as_table_mut() {
        Some(table) => {
            if explicit_after {
                table.insert("bind".into(), toml::Value::String(target.to_string()));
            } else {
                table.remove("bind");
            }
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
    handle.set_explicit(explicit_after);
    tracing::info!(addr = %target, explicit = explicit_after, "listen rebound");

    Json(PutListenResponse {
        addr: target.to_string(),
        rebound: true,
        reason: if explicit_after { None } else { Some("auto".into()) },
    })
    .into_response()
}

/// `GET /v1/admin/listen` —— 当前实际监听地址（查询用，admin 门内）。
pub async fn get_listen(State(state): State<AppState>) -> Response {
    match state.listen.get() {
        Some(h) => Json(GetListenResponse {
            addr: h.bound().to_string(),
        })
        .into_response(),
        None => err(
            &state,
            StatusCode::SERVICE_UNAVAILABLE,
            "listen supervisor unavailable",
        ),
    }
}

/// `PUT /v1/admin/password` —— 密码热更（**门外端点，in-handler 鉴权**）：
/// - **local**：loopback peer（None=oneshot 信任）否则 403；
/// - **cloud**：有效 admin 会话 **或** loopback peer（本机=root 等价，Q4 本地信任
///   —— 服务器忘密恢复 = SSH 上机一行 curl）；非 loopback 无会话 → 401。
///
/// 行为统一走 [`set_admin_password`]：写 toml + 运行时生效 + 会话全废（代际+1）
/// + env 活跃警示。**响应不含密码**。
pub async fn put_password(
    State(state): State<AppState>,
    connect: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    use crate::auth::{addr_is_loopback, bearer_token, current_mode, has_valid_session};
    // in-handler 鉴权（password 不在 admin 中间件门内 —— login 同理）
    let loopback = addr_is_loopback(connect.as_ref().map(|axum::extract::ConnectInfo(a)| a));
    match current_mode(&state) {
        crate::config::RunMode::Local => {
            if !loopback {
                return crate::auth::forbidden(&state, "local mode: loopback only");
            }
        }
        crate::config::RunMode::Cloud => {
            if !loopback && !has_valid_session(&state, bearer_token(&headers)) {
                return crate::auth::unauthorized_pub(
                    &state,
                    "admin session required (POST /v1/admin/login)",
                );
            }
        }
    }

    let req: PutPasswordBody = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return err(&state, StatusCode::BAD_REQUEST, format!("invalid password body: {e}"))
        }
    };
    if req.password.trim().is_empty() {
        return err(&state, StatusCode::BAD_REQUEST, "password must not be empty");
    }
    if let Err(r) = set_admin_password(&state, &req.password) {
        return r;
    }
    Json(PutPasswordResponse {
        updated: true,
        env_override_active: state.auth.env_password_override.load(Ordering::SeqCst),
    })
    .into_response()
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

    /* ── GET /v1/admin/status：7 键形状冻结 + 无密码值 ──────────── */

    #[tokio::test]
    async fn status_shape_frozen_seven_keys_without_password_value() {
        let cfg = format!(
            r#"
mode = "local"
admin_password = "{FAKE_KEY_PLACEHOLDER}"
[providers.laya]
kind = "laya"
base = "http://127.0.0.1:18765/v1/systemone"
enabled = true
"#,
            // 密码字段用独立假值（FAKE_KEY 是 key 形态，密码另起）
            FAKE_KEY_PLACEHOLDER = "pw-test-status-1"
        );
        let path = temp_config("status-shape", &cfg);
        let (app, _state) = app_at(path.clone());

        let (status, body) = send(app, "GET", "/v1/admin/status", None).await;
        assert_eq!(status, 200, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let obj = v.as_object().expect("status 是对象");
        // 恰 7 键（字段名冻结 —— UI 仪表盘契约）
        assert_eq!(obj.len(), 7, "7 键冻结: {body}");
        for k in [
            "mode",
            "bind",
            "bind_explicit",
            "env_override_active",
            "password_set",
            "version",
            "uptime_s",
        ] {
            assert!(obj.contains_key(k), "缺键 {k}: {body}");
        }
        assert_eq!(v["mode"], "local");
        // oneshot 无 supervisor → bind 回退配置解析值（成对默认 local）
        assert_eq!(v["bind"], "127.0.0.1:11435", "{body}");
        assert_eq!(v["bind_explicit"], false, "{body}");
        assert_eq!(v["env_override_active"], false, "{body}");
        assert_eq!(v["password_set"], true, "已配密码 → true: {body}");
        assert!(v["version"].is_string(), "{body}");
        assert!(v["uptime_s"].is_u64(), "{body}");
        // 绝不携带密码值（红线：序列化面无 password 字段名/值）
        assert!(!body.contains("pw-test-status-1"), "泄露密码值: {body}");
        assert!(obj.get("password").is_none() && obj.get("admin_password").is_none());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn status_password_set_false_when_unset() {
        let path = temp_config("status-nopw", "# empty\n");
        let (app, _state) = app_at(path.clone());
        let (status, body) = send(app, "GET", "/v1/admin/status", None).await;
        assert_eq!(status, 200, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["password_set"], false, "{body}");
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
