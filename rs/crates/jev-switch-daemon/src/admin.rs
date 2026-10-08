//! Admin API（A7 · contracts/05 §1/§2 + contracts/04 防偷红线）。
//!
//! 端点：
//! - `GET  /v1/admin/providers` → `{providers:[{id,kind,base,enabled,api_key_masked,api_key_set}]}`
//!   **只出掩码**（`sk-****a1b2` 形态），无任何读回明文的字段/路径（红线 2/3）
//! - `PUT/DELETE /v1/admin/providers/{id}` → 单 provider 原子写入/删除；批量变更只走显式确认导入
//!   已失去上游终点的路由分支；响应回 masked（红线 7）。
//!   `api_key` 省略 = 保留原 key；显式空串 = 清除（回退 `api_key_env`）
//! - `GET  /v1/admin/routes` → `{routes:[RouteEdge]}`（运行时边表 —— 含旧 `[router]`
//!   合并结果，config 载入边集合作为真值）
//! - `POST /v1/admin/routes/transaction` → 原子 create/update/delete 事务；整表 PUT 不暴露
//!   → 在副本上校验 → 单次落盘 → `Registry::replace_edges` 热替换（无重启）
//! - `POST /v1/admin/providers/{id}/probe` → `{ok,latency_ms,status,error}`
//! - `POST /v1/admin/providers/{id}/models` → 从上游模型目录获取模型 ID（不运行推理）
//! - `GET /v1/admin/capabilities` → UI/Agent 配置能力发现（只读）
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
//! - `POST /v1/admin/endpoints` → 创建服务入口 + 可选边级路由操作
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
//! - providers PUT/DELETE：只修改目标 provider 及受影响路由；批量变更只走显式确认导入
//! - route transaction：只修改 SQLite runtime snapshot；不改写 TOML，也不接受整表 PUT。
//!   旧扁平映射只在显式导入/兼容加载时处理。**注释不随 toml 往返保留**
//!   （toml crate 不保注释 —— A7 报告备案项）。
//! - mode/password/listen PUT：写对应单键，其余段原样（同上注释不保）。

pub mod endpoints;

use crate::config::{
    enforce_config_perms, enforce_dir_perms, Config, ProcessPolicy, ProviderConfig,
    ProviderLifecycleConfig,
};
use crate::{error_response, known_keys_snapshot, AppState};
use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jev_core::{
    redact::{mask_key, redact},
    router::{check_acyclic, MatchMode, RouteEdge, RouterError},
};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/* ══════════════════════════════════════════════════════════════════
DTO（ts-rs 导出 → ui/src/generated/；RouteEdge 来自 jev-core 一份真值）
══════════════════════════════════════════════════════════════════ */

/// GET/PUT provider 响应视图 —— **字段集即红线 2 字面**：无 `api_key`。
/// `api_key_masked` 仅返回 `sk-****a1b2` 一类掩码；没有有效 key 时返回空串，需结合 `api_key_set` 判断。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderView {
    pub id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    pub account: Option<String>,
    pub kind: String,
    pub base: String,
    pub models: Vec<String>,
    pub enabled: bool,
    /// Whether this provider is allowed to receive request extensions such as media.
    pub forward_extensions: bool,
    // 掩码形态 `sk-****a1b2`；无有效 key 时为 `""`（配 `api_key_set=false` 看）。
    pub api_key_masked: String,
    pub api_key_set: bool,
    /// Advanced lifecycle configuration summary; command bodies are never returned.
    pub lifecycle: ProviderLifecycleView,
}

/// Non-secret lifecycle summary exposed to the provider page and Agent discovery.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderLifecycleView {
    pub controllable: bool,
    pub process_policy: ProcessPolicy,
    pub mode: String,
    pub configured: bool,
    pub service_state: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub timeout_ms: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub readiness_timeout_ms: u64,
}

/// `GET /v1/admin/providers` 响应体。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProvidersDoc {
    pub providers: Vec<ProviderView>,
}

/// 单 provider 写入入参（**仅写入用，永不作响应**）。
/// 不 derive `Serialize` —— 从结构上杜绝「明文 key 被序列化出去」的路径。
/// `api_key` 省略时保留原 key，空串表示清除并回退 `api_key_env`；`api_key_env` 省略时保留已有值。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderInput {
    pub id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    #[serde(default)]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    #[serde(default)]
    pub account: Option<String>,
    pub kind: String,
    pub base: String,
    pub enabled: bool,
    #[serde(default)]
    pub models: Vec<String>,
    // 新明文 key；省略表示保留原 key，空串表示清除并回退到环境变量。
    #[serde(default)]
    pub api_key: Option<String>,
    // 环境变量名可选；省略表示保留已有配置。
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Omitted keeps the current setting; true is required for multimodal gateways.
    #[serde(default)]
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    pub forward_extensions: Option<bool>,
    /// Omitted preserves the existing lifecycle configuration.
    #[serde(default)]
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    pub lifecycle: Option<ProviderLifecycleConfig>,
}

/// Registered configuration capabilities exposed to human and Agent clients.
/// This is discovery metadata only; high-risk actions keep their domain endpoints.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ConfigCapability {
    pub key: String,
    pub resource: String,
    pub read_scope: String,
    pub write_scope: Option<String>,
    pub execute_scope: Option<String>,
    pub risk: String,
    pub confirmation: Option<String>,
    pub audit: bool,
    pub schema_version: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CapabilitiesDoc {
    pub capabilities: Vec<ConfigCapability>,
}

/// Lifecycle endpoints use a stable, machine-readable error surface so an
/// Agent can distinguish a disabled host-command gate from a readiness failure.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct LifecycleErrorBody {
    pub code: String,
    pub message: String,
    pub remediation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderLifecycleDoc {
    pub provider_id: String,
    /// Returned only from the dedicated admin lifecycle endpoint. The ordinary
    /// provider list continues to expose a command-free summary.
    pub command: ProviderLifecycleConfig,
    pub config: ProviderLifecycleView,
    pub status: crate::lifecycle::LifecycleStatus,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct HostCommandControlResponse {
    pub enabled: bool,
    pub shell_enabled: bool,
    pub requires_confirmation: bool,
}

/// `GET /v1/admin/capabilities` — stable discovery metadata for UI/Agent clients.
pub async fn get_capabilities() -> Json<CapabilitiesDoc> {
    Json(CapabilitiesDoc {
        capabilities: vec![
            ConfigCapability {
                key: "providers.lifecycle.controllable".into(),
                resource: "provider".into(),
                read_scope: "config:read".into(),
                write_scope: Some("provider:lifecycle:configure".into()),
                execute_scope: None,
                risk: "host_command".into(),
                confirmation: Some("cooldown_5s".into()),
                audit: true,
                schema_version: 1,
            },
            ConfigCapability {
                key: "security.allow_host_commands".into(),
                resource: "instance".into(),
                read_scope: "config:read".into(),
                write_scope: Some("host_commands:enable".into()),
                execute_scope: None,
                risk: "host_command".into(),
                confirmation: Some("cooldown_5s".into()),
                audit: true,
                schema_version: 1,
            },
            ConfigCapability {
                key: "security.allow_shell_commands".into(),
                resource: "instance".into(),
                read_scope: "config:read".into(),
                write_scope: Some("shell_commands:enable".into()),
                execute_scope: None,
                risk: "shell_command".into(),
                confirmation: Some("cooldown_5s".into()),
                audit: true,
                schema_version: 1,
            },
            ConfigCapability {
                key: "providers.lifecycle.service".into(),
                resource: "provider".into(),
                read_scope: "config:read".into(),
                write_scope: Some("provider:lifecycle:configure".into()),
                execute_scope: Some("provider:lifecycle:execute".into()),
                risk: "host_command".into(),
                confirmation: Some("cooldown_5s".into()),
                audit: true,
                schema_version: 1,
            },
        ],
    })
}

pub async fn get_host_commands(State(state): State<AppState>) -> Response {
    match load_config(&state) {
        Ok(config) => Json(HostCommandControlResponse {
            enabled: config.allow_host_commands,
            shell_enabled: config.allow_shell_commands,
            requires_confirmation: true,
        })
        .into_response(),
        Err(response) => response,
    }
}

pub async fn put_host_commands(State(state): State<AppState>, body: Bytes) -> Response {
    #[derive(serde::Deserialize)]
    struct Input {
        enabled: bool,
        #[serde(default)]
        shell_enabled: Option<bool>,
    }
    let input: Input = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => {
            return lifecycle_error(
                &state,
                StatusCode::BAD_REQUEST,
                &format!("invalid_host_command_body:{error}"),
            )
        }
    };
    let mut config = match load_config(&state) {
        Ok(config) => config,
        Err(response) => return response,
    };
    config.allow_host_commands = input.enabled;
    if let Some(shell_enabled) = input.shell_enabled {
        config.allow_shell_commands = shell_enabled;
    }
    {
        let conn = match state.db_conn.lock() {
            Ok(conn) => conn,
            Err(error) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("database lock failed: {error}"),
                )
            }
        };
        if let Err(error) = crate::db::persist_runtime_config(&conn, &config, &state.config_path) {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("save host command setting failed: {error}"),
            );
        }
    }
    state.lifecycle.configure(&config);
    Json(HostCommandControlResponse {
        enabled: input.enabled,
        shell_enabled: config.allow_shell_commands,
        requires_confirmation: true,
    })
    .into_response()
}

pub async fn provider_lifecycle_status(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let cfg = match load_config(&state) {
        Ok(cfg) => cfg,
        Err(response) => return response,
    };
    let Some(provider) = cfg.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };
    Json(state.lifecycle.status(&id, provider, &cfg).await).into_response()
}

pub async fn get_provider_lifecycle(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let cfg = match load_config(&state) {
        Ok(cfg) => cfg,
        Err(response) => return response,
    };
    let Some(provider) = cfg.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };
    let status = state.lifecycle.status(&id, provider, &cfg).await;
    Json(ProviderLifecycleDoc {
        provider_id: id,
        command: provider.lifecycle.clone(),
        config: lifecycle_view(provider, &status),
        status,
    })
    .into_response()
}

pub async fn put_provider_lifecycle(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    body: Bytes,
) -> Response {
    let lifecycle: ProviderLifecycleConfig = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return lifecycle_error(
                &state,
                StatusCode::BAD_REQUEST,
                &format!("invalid_lifecycle_body:{error}"),
            )
        }
    };
    if let Err(reason) = lifecycle.validate() {
        return lifecycle_error(
            &state,
            StatusCode::BAD_REQUEST,
            &format!("invalid_lifecycle_configuration:{reason}"),
        );
    }
    let mut cfg = match load_config(&state) {
        Ok(cfg) => cfg,
        Err(response) => return response,
    };
    let Some(provider) = cfg.providers.get_mut(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };
    provider.lifecycle = lifecycle;
    {
        let conn = match state.db_conn.lock() {
            Ok(conn) => conn,
            Err(error) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("database lock failed: {error}"),
                )
            }
        };
        if let Err(error) = crate::db::persist_runtime_config(&conn, &cfg, &state.config_path) {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("save lifecycle configuration failed: {error}"),
            );
        }
    }
    state.lifecycle.configure(&cfg);
    let provider = cfg.providers.get(&id).expect("provider was just validated");
    let status = state.lifecycle.status(&id, provider, &cfg).await;
    Json(ProviderLifecycleDoc {
        provider_id: id,
        command: provider.lifecycle.clone(),
        config: lifecycle_view(provider, &status),
        status,
    })
    .into_response()
}

pub async fn provider_lifecycle_start(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let cfg = match load_config(&state) {
        Ok(cfg) => cfg,
        Err(response) => return response,
    };
    let Some(provider) = cfg.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };
    match state.lifecycle.start(&id, provider, &cfg).await {
        Ok(status) => Json(status).into_response(),
        Err(message) => lifecycle_error(&state, StatusCode::BAD_REQUEST, &message),
    }
}

pub async fn provider_lifecycle_stop(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let cfg = match load_config(&state) {
        Ok(cfg) => cfg,
        Err(response) => return response,
    };
    let Some(provider) = cfg.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };
    match state.lifecycle.stop(&id, provider, &cfg).await {
        Ok(status) => Json(status).into_response(),
        Err(message) => lifecycle_error(&state, StatusCode::BAD_REQUEST, &message),
    }
}

/// `GET /v1/admin/routes` 响应（`RouteEdge` = jev-core 导出，一份真值）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct RoutesDoc {
    pub routes: Vec<jev_core::router::RouteEdge>,
}

/// 原子路由事务。客户端只提交要发生的边操作，不提交完整边表。
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RouteMutation {
    Create {
        route: jev_core::router::RouteEdge,
    },
    Update {
        edge_id: String,
        route: jev_core::router::RouteEdge,
    },
    Delete {
        edge_id: String,
    },
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteTransactionRequest {
    pub operations: Vec<RouteMutation>,
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

/// 上游模型目录发现结果。响应只含模型公开元数据，不含地址或密钥。
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ProviderModelsResult {
    pub ok: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub latency_ms: u64,
    /// `0` = 没有收到 HTTP 响应；否则为上游真实 HTTP 状态。
    pub status: u16,
    pub models: Vec<String>,
    pub error: Option<String>,
}

/// 未保存表单的模型发现请求。仅在请求生命周期内携带明文 key，不落盘、不回显。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProviderModelsInput {
    pub kind: String,
    pub base: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
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

fn lifecycle_error(state: &AppState, status: StatusCode, raw: &str) -> Response {
    let (raw_code, raw_detail) = raw
        .split_once(':')
        .map(|(code, detail)| (code, Some(detail.trim().to_string())))
        .unwrap_or((raw, None));
    let code = raw_code.trim().to_string();
    let (message, remediation) = match code.as_str() {
        "invalid_lifecycle_body" => (
            "The lifecycle configuration could not be read.",
            "Check that the form contains valid values, then save again.",
        ),
        "invalid_lifecycle_configuration" => (
            "The lifecycle configuration is invalid.",
            "Correct the highlighted configuration and save it before running a service action.",
        ),
        "host_commands_disabled" => (
            "Host command execution is disabled for this instance.",
            "An administrator must enable host commands after the safety confirmation.",
        ),
        "provider_not_controllable" => (
            "This provider is configured as an uncontrolled service.",
            "Enable the provider's controllable-service setting before executing lifecycle actions.",
        ),
        "command_not_configured" => (
            "The requested lifecycle command is not configured.",
            "Configure the provider-level command or leave the action under manual control.",
        ),
        "stop_not_configured" => (
            "No stop command is configured for this provider.",
            "Configure an explicit stop command for services that start child processes, or switch to a managed executable with a verified process policy.",
        ),
        "command_failed" => (
            "The lifecycle command could not be started or completed.",
            "Check the executable path, arguments, working directory, and the service's own logs.",
        ),
        "program_path_must_be_absolute" => (
            "The lifecycle program path must be absolute.",
            "Use a full path such as C:\\Tools\\Laya\\laya.exe or /usr/local/bin/laya.",
        ),
        "shell_execution_not_allowed" => (
            "Shell lifecycle commands are disabled for this instance.",
            "Enable shell lifecycle commands in Settings after the separate safety confirmation, or point the configuration at the real executable.",
        ),
        "secrets_unavailable" => (
            "The provider API key is not available for this command.",
            "Configure the provider key or turn off API key injection before starting the service.",
        ),
        "secret_placeholder_requires_environment" => (
            "API keys cannot be inserted into command arguments.",
            "Use the injected JEV_PROVIDER_API_KEY environment variable only when you trust the command.",
        ),
        "unknown_lifecycle_placeholder" => (
            "The lifecycle command contains an unknown placeholder.",
            "Remove the placeholder or use one of the documented JEV_PROVIDER_* variables.",
        ),
        "command_timeout" | "status_command_timeout" => (
            "The lifecycle command exceeded its time limit.",
            "Check the command and service logs, then increase the timeout only when necessary.",
        ),
        "status_command_failed" => (
            "The provider status command could not be executed.",
            "Check the status program path and arguments, and use exit code 0 for running or 3 for stopped.",
        ),
        "process_lost" | "readiness_process_exited" => (
            "The service process exited before it became ready.",
            "Run the same command manually, inspect its logs, and verify the provider health address.",
        ),
        "process_status_failed" => (
            "The service process status could not be read.",
            "Check local process permissions and the configured process policy.",
        ),
        "readiness_not_configured" => (
            "The service has no usable readiness check.",
            "Configure a status command or a provider address whose /health endpoint can be checked.",
        ),
        "readiness_timeout" => (
            "The service command started but did not become ready before the bounded timeout.",
            "Check the provider address, status command, service logs, and readiness timeout.",
        ),
        "ownership_unverified" => (
            "The recorded process could not be verified as owned by Jev Switch.",
            "Use the provider's explicit stop command or inspect the service manually; no PID was terminated.",
        ),
        "lifecycle_operation_in_progress" => (
            "Another lifecycle action for this provider is still running.",
            "Wait for the current action to finish before retrying.",
        ),
        _ => (
            "The provider lifecycle action failed.",
            "Inspect the lifecycle status and audit details, then correct the provider configuration.",
        ),
    };
    let body = LifecycleErrorBody {
        code,
        message: message.into(),
        remediation: remediation.into(),
        detail: raw_detail.filter(|detail| !detail.is_empty()),
    };
    let mut response = (status, Json(body)).into_response();
    if let Ok(request_id) = HeaderValue::from_str(&format!("lifecycle-{}", lifecycle_request_id()))
    {
        response
            .headers_mut()
            .insert("x-jev-request-id", request_id);
    }
    let _ = state; // keep the helper's redaction boundary explicit at the handler call site
    response
}

fn lifecycle_request_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[allow(clippy::result_large_err)]
fn load_config_from_conn(
    state: &AppState,
    conn: &rusqlite::Connection,
) -> Result<Config, Response> {
    let mut config = Config::load(&state.config_path).map_err(|e| {
        err_plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("config load failed: {e}"),
        )
    })?;
    crate::db::restore_or_seed_runtime_config(conn, &mut config, &state.config_path).map_err(
        |e| {
            err(
                state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("runtime snapshot load failed: {e}"),
            )
        },
    )?;
    Ok(config)
}

#[allow(clippy::result_large_err)]
pub(crate) fn load_config(state: &AppState) -> Result<Config, Response> {
    let conn = state.db_conn.lock().map_err(|e| {
        err(
            state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("database lock failed: {e}"),
        )
    })?;
    load_config_from_conn(state, &conn)
}

/// 刷新已知密钥集（provider 读写后调用 —— 外部手改的 key 也纳入 redact）。
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
        name: p.name.clone(),
        account: p.account.clone(),
        kind: p.kind.clone(),
        // 防御性再过一遍 redact（正常 URL 不含 key；防 base 被人为贴 key 的边角）
        base: redact(&p.base, keys),
        models: p.models.clone(),
        enabled: p.enabled,
        forward_extensions: p.forward_extensions,
        api_key_masked,
        api_key_set,
        lifecycle: lifecycle_view(p, &unknown_lifecycle_status(id)),
    }
}

fn unknown_lifecycle_status(provider_id: &str) -> crate::lifecycle::LifecycleStatus {
    crate::lifecycle::LifecycleStatus {
        provider_id: provider_id.to_string(),
        state: "unknown".into(),
        readiness: "unknown".into(),
        controllable: false,
        configured: false,
        managed_by_jev: false,
        pid: None,
        execution_id: None,
        message: None,
    }
}

fn lifecycle_view(
    provider: &ProviderConfig,
    status: &crate::lifecycle::LifecycleStatus,
) -> ProviderLifecycleView {
    ProviderLifecycleView {
        controllable: provider.lifecycle.controllable,
        process_policy: provider.lifecycle.process_policy.clone(),
        mode: provider.lifecycle.mode.clone(),
        configured: provider.lifecycle.program.is_some()
            || provider.lifecycle.stop_program.is_some()
            || provider.lifecycle.status_program.is_some(),
        service_state: status.state.clone(),
        timeout_ms: provider.lifecycle.timeout_ms,
        readiness_timeout_ms: provider.lifecycle.readiness_timeout_ms,
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
#[allow(clippy::result_large_err)]
pub(crate) fn read_value(path: &Path) -> Result<toml::Value, Response> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        err_plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("config read failed: {e}"),
        )
    })?;
    raw.parse::<toml::Value>().map_err(|e| {
        err_plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("config parse failed: {e}"),
        )
    })
}

/// 落盘 + 权限收紧（0600 文件；0700 仅限默认配置目录 `~/.jev-switch` ——
/// `JEV_SWITCH_CONFIG` 可能指向仓库内示例，不乱 chmod 其父目录）。
#[allow(clippy::result_large_err)]
pub(crate) fn write_value(path: &Path, value: &toml::Value) -> Result<(), Response> {
    let s = toml::to_string(value).map_err(|e| {
        err_plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("config encode failed: {e}"),
        )
    })?;
    std::fs::write(path, s).map_err(|e| {
        err_plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("config write failed: {e}"),
        )
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

fn validate_provider_input(input: &ProviderInput) -> Result<(), String> {
    if input.id.trim().is_empty() {
        return Err("provider id 不得为空".into());
    }
    if input.kind.trim().is_empty() {
        return Err(format!("provider '{}' kind 不得为空", input.id));
    }
    if !matches!(
        input.kind.as_str(),
        "vercel" | "laya" | "typesafe" | "openrouter"
    ) {
        return Err(format!(
            "provider '{}' kind '{}' 不受支持；可选值：vercel、laya、typesafe、openrouter",
            input.id, input.kind
        ));
    }
    if input.base.trim().is_empty() {
        return Err(format!("provider '{}' base 不得为空", input.id));
    }
    Ok(())
}

fn provider_config_from_input(
    input: &ProviderInput,
    old: Option<&ProviderConfig>,
) -> ProviderConfig {
    ProviderConfig {
        kind: input.kind.clone(),
        base: input.base.clone(),
        name: input
            .name
            .clone()
            .or_else(|| old.and_then(|v| v.name.clone()))
            .filter(|v| !v.trim().is_empty()),
        account: input
            .account
            .clone()
            .or_else(|| old.and_then(|v| v.account.clone()))
            .filter(|v| !v.trim().is_empty()),
        models: input
            .models
            .iter()
            .map(|model| model.trim())
            .filter(|model| !model.is_empty())
            .map(String::from)
            .collect(),
        api_key: match &input.api_key {
            Some(value) if value.is_empty() => None,
            Some(value) => Some(value.clone()),
            None => old.and_then(|v| v.api_key.clone()),
        },
        api_key_env: input
            .api_key_env
            .clone()
            .or_else(|| old.and_then(|v| v.api_key_env.clone())),
        enabled: input.enabled,
        forward_extensions: input
            .forward_extensions
            .or_else(|| old.map(|v| v.forward_extensions))
            .unwrap_or(false),
        lifecycle: input
            .lifecycle
            .clone()
            .or_else(|| old.map(|v| v.lifecycle.clone()))
            .unwrap_or_default(),
    }
}

fn prepare_provider_config(
    cfg: &mut Config,
    previous: &HashMap<String, ProviderConfig>,
) -> Result<(), String> {
    for (provider_id, provider) in &cfg.providers {
        provider
            .lifecycle
            .validate()
            .map_err(|reason| format!("invalid_lifecycle_configuration:{provider_id}:{reason}"))?;
    }
    let removed: BTreeSet<String> = previous
        .keys()
        .filter(|id| !cfg.providers.contains_key(*id))
        .cloned()
        .collect();
    cfg.router.clear();
    prune_routes_for_removed_providers(&mut cfg.routes, &cfg.providers, &removed);
    validate_route_graph(&cfg.routes, &cfg.providers)
        .map_err(|message| format!("provider update rejected: {message}"))
}

fn finish_provider_config_update(state: &AppState, cfg: Config) -> Response {
    state.lifecycle.configure(&cfg);
    refresh_known_keys(state, &cfg);
    state
        .registry
        .replace_upstreams(crate::build_runtime_upstreams_with_lifecycle(
            &cfg,
            &state.telemetry,
            state.lifecycle.clone(),
        ));
    if let Err(message) = crate::admin::endpoints::refresh_endpoint_routes(state, &[]) {
        return err(
            state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("refresh merged route graph after provider update failed: {message}"),
        );
    }
    let keys = known_keys_snapshot(state);
    Json(providers_doc(&cfg, &keys)).into_response()
}

fn provider_update_error(state: &AppState, message: String) -> Response {
    if message.starts_with("invalid_lifecycle_configuration:") {
        lifecycle_error(state, StatusCode::BAD_REQUEST, &message)
    } else {
        err(state, StatusCode::BAD_REQUEST, message)
    }
}

/// Update exactly one provider in the current SQLite snapshot.
pub async fn put_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    body: Bytes,
) -> Response {
    let input: ProviderInput = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid body: {error}"),
            )
        }
    };
    if input.id != id {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "provider path id and body id must match",
        );
    }
    if let Err(message) = validate_provider_input(&input) {
        return err(&state, StatusCode::BAD_REQUEST, message);
    }
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {error}"),
            )
        }
    };
    let mut config = match load_config_from_conn(&state, &conn) {
        Ok(config) => config,
        Err(response) => return response,
    };
    let previous = config.providers.clone();
    let old = config.providers.get(&id);
    let provider = provider_config_from_input(&input, old);
    config.providers.insert(id.clone(), provider);
    if let Err(message) = prepare_provider_config(&mut config, &previous) {
        return provider_update_error(&state, message);
    }
    if let Err(error) = crate::db::persist_runtime_config(&conn, &config, &state.config_path) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save provider snapshot failed: {error}"),
        );
    }
    drop(conn);
    finish_provider_config_update(&state, config)
}

/// Delete exactly one provider in the current SQLite snapshot.
pub async fn delete_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {error}"),
            )
        }
    };
    let mut config = match load_config_from_conn(&state, &conn) {
        Ok(config) => config,
        Err(response) => return response,
    };
    if !config.providers.contains_key(&id) {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    }
    let previous = config.providers.clone();
    config.providers.remove(&id);
    if let Err(message) = prepare_provider_config(&mut config, &previous) {
        return err(&state, StatusCode::BAD_REQUEST, message);
    }
    if let Err(error) = crate::db::persist_runtime_config(&conn, &config, &state.config_path) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save provider snapshot failed: {error}"),
        );
    }
    drop(conn);
    finish_provider_config_update(&state, config)
}

/* ══════════════════════════════════════════════════════════════════
GET / POST · routes
══════════════════════════════════════════════════════════════════ */

/// 运行时边表（含旧 `[router]` 合并结果 —— config 载入边集合作为真值）。
pub async fn get_routes(State(state): State<AppState>) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {e}"),
            )
        }
    };
    match crate::db::load_runtime_snapshot(&conn) {
        Ok(Some(snapshot)) => Json(RoutesDoc {
            routes: snapshot.routes,
        })
        .into_response(),
        Ok(None) => err(
            &state,
            StatusCode::CONFLICT,
            "runtime config snapshot is missing",
        ),
        Err(e) => err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("route snapshot read failed: {e}"),
        ),
    }
}

/// Read-only canonical graph projection for the node-engine migration phase.
/// Existing `/v1/admin/routes` remains the compatibility edge view.
pub async fn get_graph(State(state): State<AppState>) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {error}"),
            )
        }
    };
    let mut snapshot = match crate::db::load_runtime_snapshot(&conn) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            return err(
                &state,
                StatusCode::CONFLICT,
                "runtime config snapshot is missing",
            )
        }
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load graph snapshot failed: {error}"),
            )
        }
    };
    if snapshot.nodes.is_empty() {
        snapshot.nodes =
            match crate::db::derive_runtime_nodes(&conn, &snapshot.routes, &snapshot.providers) {
                Ok(nodes) => nodes,
                Err(error) => {
                    return err(
                        &state,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("derive graph nodes failed: {error}"),
                    )
                }
            };
    }
    Json(jev_core::graph::GraphDocument {
        nodes: snapshot.nodes,
        edges: snapshot.routes,
    })
    .into_response()
}

/// Every route write validates the complete resulting graph before committing it.
/// Disabled providers remain valid configuration references; execution checks their state.
/// Validation order: fields, cycles, then references.
pub(crate) fn validate_route_graph(
    routes: &[jev_core::router::RouteEdge],
    providers: &HashMap<String, ProviderConfig>,
) -> Result<(), String> {
    for e in routes {
        if e.left.trim().is_empty() || e.right.trim().is_empty() {
            return Err("route field invalid: left/right 不得为空".into());
        }
    }
    if let Err(e) = check_acyclic(routes) {
        return Err(match &e {
            RouterError::Cycle(path) => format!("路由配置存在环 (cycle): {path}"),
            other => other.to_string(),
        });
    }
    let alias_lefts: BTreeSet<&str> = routes
        .iter()
        .filter(|e| e.r#match == MatchMode::Exact)
        .map(|e| e.left.as_str())
        .collect();
    for e in routes {
        if providers.contains_key(&e.right) || alias_lefts.contains(e.right.as_str()) {
            continue;
        }
        return Err(format!(
            "route invalid: 边 '{}' 的 right '{}' 既不是已注册 provider，也不是新表中的别名节点",
            e.left, e.right
        ));
    }
    Ok(())
}

fn route_edge_id(edge: &RouteEdge) -> String {
    let match_mode = match edge.r#match {
        MatchMode::Exact => "exact",
        MatchMode::Prefix => "prefix",
    };
    serde_json::to_string(&(
        edge.left.as_str(),
        edge.right.as_str(),
        match_mode,
        edge.upstream_model.as_deref(),
    ))
    .expect("route edge identity must serialize")
}

/// Resolve the canonical JSON edge identity; the old `left=>right` form is
/// accepted only when it identifies one unambiguous legacy edge.
fn route_edge_index(routes: &[RouteEdge], edge_id: &str) -> Option<usize> {
    if let Some(index) = routes
        .iter()
        .position(|edge| route_edge_id(edge) == edge_id)
    {
        return Some(index);
    }
    let legacy_matches: Vec<usize> = routes
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            let by_provider = format!("{}=>{}", edge.left, edge.right);
            let by_model = edge
                .upstream_model
                .as_ref()
                .map(|model| format!("{}=>{}", edge.left, model));
            (edge_id == by_provider || by_model.as_deref() == Some(edge_id)).then_some(index)
        })
        .collect();
    if legacy_matches.len() == 1 {
        Some(legacy_matches[0])
    } else {
        None
    }
}

#[derive(Debug)]
pub(crate) enum RouteMutationFailure {
    Conflict(String),
    NotFound(String),
}

pub(crate) fn apply_route_mutations(
    routes: &mut Vec<RouteEdge>,
    operations: &[RouteMutation],
) -> Result<(), RouteMutationFailure> {
    for operation in operations {
        match operation {
            RouteMutation::Create { route } => {
                let edge_id = route_edge_id(route);
                if route_edge_index(routes, &edge_id).is_some() {
                    return Err(RouteMutationFailure::Conflict(format!(
                        "route edge already exists: {edge_id}"
                    )));
                }
                routes.push(route.clone());
            }
            RouteMutation::Update { edge_id, route } => {
                let Some(index) = route_edge_index(routes, edge_id) else {
                    return Err(RouteMutationFailure::NotFound(format!(
                        "route edge not found: {edge_id}"
                    )));
                };
                let next_id = route_edge_id(route);
                let current_id = route_edge_id(&routes[index]);
                if next_id != current_id && route_edge_index(routes, &next_id).is_some() {
                    return Err(RouteMutationFailure::Conflict(format!(
                        "route edge already exists: {next_id}"
                    )));
                }
                routes[index] = route.clone();
            }
            RouteMutation::Delete { edge_id } => {
                let Some(index) = route_edge_index(routes, edge_id) else {
                    return Err(RouteMutationFailure::NotFound(format!(
                        "route edge not found: {edge_id}"
                    )));
                };
                routes.remove(index);
            }
        }
    }
    Ok(())
}

/// Apply route mutations to a private copy, validate the complete resulting graph,
/// then persist and hot-replace it once. No caller can submit an entire replacement
/// table through the public admin write surface.
pub async fn route_transaction(State(state): State<AppState>, body: Bytes) -> Response {
    let request: RouteTransactionRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid route transaction: {error}"),
            )
        }
    };
    if request.operations.is_empty() {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "route transaction requires at least one operation",
        );
    }

    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {error}"),
            )
        }
    };
    let snapshot = match crate::db::load_runtime_snapshot(&conn) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            return err(
                &state,
                StatusCode::CONFLICT,
                "runtime config snapshot is missing",
            )
        }
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("route snapshot read failed: {error}"),
            )
        }
    };
    let mut routes = snapshot.routes.clone();
    if let Err(failure) = apply_route_mutations(&mut routes, &request.operations) {
        return match failure {
            RouteMutationFailure::Conflict(message) => err(&state, StatusCode::CONFLICT, message),
            RouteMutationFailure::NotFound(message) => err(&state, StatusCode::NOT_FOUND, message),
        };
    }
    if let Err(message) = validate_route_graph(&routes, &snapshot.providers) {
        return err(&state, StatusCode::BAD_REQUEST, message);
    }
    let nodes = match crate::db::derive_runtime_nodes(&conn, &routes, &snapshot.providers) {
        Ok(nodes) => nodes,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("derive graph nodes failed: {error}"),
            )
        }
    };
    let next_snapshot = crate::db::RuntimeConfigSnapshot {
        providers: snapshot.providers,
        allow_host_commands: snapshot.allow_host_commands,
        allow_shell_commands: snapshot.allow_shell_commands,
        nodes,
        routes: routes.clone(),
        source_toml_fingerprint: snapshot.source_toml_fingerprint,
    };
    if let Err(error) = crate::db::save_runtime_snapshot(&conn, &next_snapshot) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save route transaction failed: {error}"),
        );
    }
    drop(conn);
    state.registry.replace_edges(routes.clone());
    Json(RoutesDoc { routes }).into_response()
}

/// Remove provider-owned route branches after an all-table provider replacement.
///
/// A route can point at an alias rather than directly at a provider. When the last
/// provider below such an alias is removed, retaining the parent edge would leave a
/// dangling branch that only fails later during request routing. Compute the nodes
/// that still resolve to a current provider, then keep only edges whose right side is
/// resolvable. The snapshot write and this pruning happen before the in-memory router
/// is refreshed, so a failed validation leaves both authorities unchanged.
fn prune_routes_for_removed_providers(
    routes: &mut Vec<RouteEdge>,
    providers: &HashMap<String, ProviderConfig>,
    removed_providers: &BTreeSet<String>,
) -> usize {
    if removed_providers.is_empty() {
        return 0;
    }

    let original_count = routes.len();
    let mut resolvable: BTreeSet<String> = providers.keys().cloned().collect();
    loop {
        let additions: Vec<String> = routes
            .iter()
            .filter(|edge| edge.r#match == MatchMode::Exact && resolvable.contains(&edge.right))
            .map(|edge| edge.left.clone())
            .filter(|left| !resolvable.contains(left) && !removed_providers.contains(left))
            .collect();
        if additions.is_empty() {
            break;
        }
        resolvable.extend(additions);
    }

    let mut affected = removed_providers.clone();
    loop {
        let additions: Vec<String> = routes
            .iter()
            .filter(|edge| edge.r#match == MatchMode::Exact && affected.contains(&edge.right))
            .map(|edge| edge.left.clone())
            .filter(|left| !affected.contains(left))
            .collect();
        if additions.is_empty() {
            break;
        }
        affected.extend(additions);
    }

    routes.retain(|edge| {
        !removed_providers.contains(&edge.left)
            && !removed_providers.contains(&edge.right)
            && (!affected.contains(&edge.right) || resolvable.contains(&edge.right))
    });
    original_count.saturating_sub(routes.len())
}

/// Report whether the legacy editable TOML differs from the imported SQLite source baseline.
pub async fn runtime_config_status(State(state): State<AppState>) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {e}"),
            )
        }
    };
    let snapshot = match crate::db::load_runtime_snapshot(&conn) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            return err(
                &state,
                StatusCode::CONFLICT,
                "runtime snapshot has not been initialized",
            )
        }
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("snapshot read failed: {e}"),
            )
        }
    };
    let current = crate::db::config_fingerprint(&state.config_path);
    Json(serde_json::json!({
        "authority": "sqlite",
        "toml_path": state.config_path,
        "toml_drifted": snapshot.source_toml_fingerprint != current,
        "source_fingerprint": snapshot.source_toml_fingerprint,
        "current_fingerprint": current,
        "explicit_import_endpoint": "/v1/admin/config/import-toml",
        "explicit_export_endpoint": "/v1/admin/config/export-toml"
    }))
    .into_response()
}

/// Explicitly import the current providers/routes from TOML into SQLite. The confirmation
/// body makes this distinct from normal reads or routine provider/route writes.
pub async fn import_runtime_config(State(state): State<AppState>, body: Bytes) -> Response {
    let request: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid confirmation: {e}"),
            )
        }
    };
    if request.get("confirm").and_then(serde_json::Value::as_bool) != Some(true) {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "explicit import requires {\"confirm\":true}",
        );
    }
    let config = match Config::load(&state.config_path) {
        Ok(config) => config,
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("TOML import rejected: {e}"),
            )
        }
    };
    let routes = config.route_edges();
    if let Err(message) = validate_route_graph(&routes, &config.providers) {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            format!("TOML import rejected: {message}"),
        );
    }
    // Construct adapters before changing the canonical snapshot. A provider that
    // cannot be registered must not yield a successful import with stale runtime.
    let adapters = crate::build_runtime_upstreams_with_lifecycle(
        &config,
        &state.telemetry,
        state.lifecycle.clone(),
    );
    let adapter_ids: BTreeSet<&str> = adapters.iter().map(|adapter| adapter.id()).collect();
    for (id, provider) in &config.providers {
        if provider.enabled && !adapter_ids.contains(id.as_str()) {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!(
                    "TOML import rejected: enabled provider '{id}' could not initialize an adapter"
                ),
            );
        }
    }
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {e}"),
            )
        }
    };
    let endpoints = match crate::db::endpoints::load_all(&conn) {
        Ok(endpoints) => endpoints,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load endpoints before import failed: {e}"),
            )
        }
    };
    let endpoint_ids: std::collections::HashSet<String> = endpoints
        .iter()
        .map(|endpoint| endpoint.id.clone())
        .collect();
    let enabled: std::collections::HashSet<String> = endpoints
        .into_iter()
        .filter(|endpoint| endpoint.enabled)
        .map(|endpoint| endpoint.id)
        .collect();
    let runtime_edges: Vec<_> = routes
        .iter()
        .filter(|edge| !endpoint_ids.contains(&edge.left) || enabled.contains(&edge.left))
        .cloned()
        .collect();
    let snapshot = crate::db::RuntimeConfigSnapshot {
        providers: config.providers.clone(),
        allow_host_commands: config.allow_host_commands,
        allow_shell_commands: config.allow_shell_commands,
        nodes: match crate::db::derive_runtime_nodes(&conn, &routes, &config.providers) {
            Ok(nodes) => nodes,
            Err(error) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("derive graph nodes failed: {error}"),
                )
            }
        },
        routes,
        source_toml_fingerprint: crate::db::config_fingerprint(&state.config_path),
    };
    if let Err(e) = crate::db::save_runtime_snapshot(&conn, &snapshot) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("import snapshot failed: {e}"),
        );
    }
    drop(conn);
    refresh_known_keys(&state, &config);
    state.registry.replace_upstreams(adapters);
    state.registry.replace_edges(runtime_edges);
    Json(serde_json::json!({"imported":true,"fingerprint":crate::db::config_fingerprint(&state.config_path)})).into_response()
}

const RUNTIME_BACKUP_SCHEMA_VERSION: u32 = 1;
const RUNTIME_BACKUP_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeConfigBackup {
    schema_version: u32,
    providers: HashMap<String, ProviderConfig>,
    routes: Vec<RouteEdge>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeConfigBackupImport {
    confirm: bool,
    backup: RuntimeConfigBackup,
}

fn no_store_json(value: impl Serialize) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    response.headers_mut().insert(
        axum::http::header::PRAGMA,
        HeaderValue::from_static("no-cache"),
    );
    response
}

/// Export the active provider and route snapshot only after explicit confirmation.
/// This is intentionally separate from the ordinary masked provider read API.
pub async fn export_runtime_config_json(State(state): State<AppState>, body: Bytes) -> Response {
    if body.len() > 1024 {
        return err(
            &state,
            StatusCode::PAYLOAD_TOO_LARGE,
            "confirmation body too large",
        );
    }
    let request: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid confirmation: {error}"),
            );
        }
    };
    if request.get("confirm").and_then(serde_json::Value::as_bool) != Some(true) {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "explicit export requires {\"confirm\":true}",
        );
    }
    let mut config = match load_config(&state) {
        Ok(config) => config,
        Err(response) => return response,
    };
    let effective_keys: HashMap<String, String> = config
        .providers
        .keys()
        .filter_map(|id| config.effective_api_key(id).map(|key| (id.clone(), key)))
        .collect();
    for (id, provider) in &mut config.providers {
        if let Some(key) = effective_keys.get(id) {
            provider.api_key = Some(key.clone());
            provider.api_key_env = None;
        }
    }
    let routes = config.route_edges();
    no_store_json(RuntimeConfigBackup {
        schema_version: RUNTIME_BACKUP_SCHEMA_VERSION,
        providers: config.providers,
        routes,
    })
}

/// Restore a versioned provider/route backup after explicit user confirmation.
pub async fn import_runtime_config_json(State(state): State<AppState>, body: Bytes) -> Response {
    if body.len() > RUNTIME_BACKUP_MAX_BYTES {
        return err(
            &state,
            StatusCode::PAYLOAD_TOO_LARGE,
            "configuration backup exceeds 2 MiB",
        );
    }
    let request: RuntimeConfigBackupImport = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid configuration backup: {error}"),
            )
        }
    };
    if !request.confirm {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "explicit import requires confirm=true",
        );
    }
    if request.backup.schema_version != RUNTIME_BACKUP_SCHEMA_VERSION {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "unsupported configuration backup schema_version",
        );
    }

    let mut config = match load_config(&state) {
        Ok(config) => config,
        Err(response) => return response,
    };
    config.providers = request.backup.providers;
    config.routes = request.backup.routes;
    config.router.clear();
    if let Err(error) = config.validate_routes() {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            format!("configuration backup rejected: {error}"),
        );
    }
    if let Err(message) = validate_route_graph(&config.route_edges(), &config.providers) {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            format!("configuration backup rejected: {message}"),
        );
    }
    for (id, provider) in &config.providers {
        if id.trim().is_empty()
            || provider.kind.trim().is_empty()
            || provider.base.trim().is_empty()
        {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                "configuration backup contains an incomplete provider",
            );
        }
    }

    let adapters = crate::build_runtime_upstreams_with_lifecycle(
        &config,
        &state.telemetry,
        state.lifecycle.clone(),
    );
    let adapter_ids: BTreeSet<&str> = adapters.iter().map(|adapter| adapter.id()).collect();
    for (id, provider) in &config.providers {
        if provider.enabled && !adapter_ids.contains(id.as_str()) {
            return err(&state, StatusCode::BAD_REQUEST, format!("configuration backup rejected: enabled provider '{id}' could not initialize an adapter"));
        }
    }

    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {error}"),
            )
        }
    };
    let endpoints = match crate::db::endpoints::load_all(&conn) {
        Ok(endpoints) => endpoints,
        Err(error) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load endpoints before import failed: {error}"),
            )
        }
    };
    let endpoint_ids: std::collections::HashSet<String> = endpoints
        .iter()
        .map(|endpoint| endpoint.id.clone())
        .collect();
    let enabled: std::collections::HashSet<String> = endpoints
        .into_iter()
        .filter(|endpoint| endpoint.enabled)
        .map(|endpoint| endpoint.id)
        .collect();
    let routes = config.route_edges();
    let runtime_edges = routes
        .iter()
        .filter(|edge| !endpoint_ids.contains(&edge.left) || enabled.contains(&edge.left))
        .cloned()
        .collect();
    let snapshot = crate::db::RuntimeConfigSnapshot {
        providers: config.providers.clone(),
        allow_host_commands: config.allow_host_commands,
        allow_shell_commands: config.allow_shell_commands,
        nodes: match crate::db::derive_runtime_nodes(&conn, &routes, &config.providers) {
            Ok(nodes) => nodes,
            Err(error) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("derive graph nodes failed: {error}"),
                )
            }
        },
        routes,
        source_toml_fingerprint: crate::db::config_fingerprint(&state.config_path),
    };
    if let Err(error) = crate::db::save_runtime_snapshot(&conn, &snapshot) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("configuration backup import failed: {error}"),
        );
    }
    drop(conn);
    refresh_known_keys(&state, &config);
    state.registry.replace_upstreams(adapters);
    state.registry.replace_edges(runtime_edges);
    no_store_json(
        serde_json::json!({"imported": true, "schema_version": RUNTIME_BACKUP_SCHEMA_VERSION}),
    )
}

/// Export the current SQLite provider/route runtime snapshot to the legacy TOML file.
pub async fn export_runtime_config(State(state): State<AppState>) -> Response {
    let config = match load_config(&state) {
        Ok(config) => config,
        Err(response) => return response,
    };
    let mut value = match read_value(&state.config_path) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(table) = value.as_table_mut() else {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            "config root is not a table",
        );
    };
    let mut providers = toml::Table::new();
    for (id, provider) in &config.providers {
        match toml::Value::try_from(provider) {
            Ok(value) => {
                providers.insert(id.clone(), value);
            }
            Err(e) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("provider export failed: {e}"),
                )
            }
        }
    }
    let mut routes = toml::value::Array::new();
    for edge in config.route_edges() {
        match toml::Value::try_from(edge) {
            Ok(value) => routes.push(value),
            Err(e) => {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("route export failed: {e}"),
                )
            }
        }
    }
    table.insert("providers".into(), toml::Value::Table(providers));
    table.remove("router");
    table.insert("routes".into(), toml::Value::Array(routes));
    let original_file = match std::fs::read(&state.config_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("config backup failed before export: {e}"),
            )
        }
    };
    let mut exported = config;
    exported.router.clear();
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database lock failed: {e}"),
            )
        }
    };
    if let Err(response) = write_value(&state.config_path, &value) {
        if let Err(restore_error) = std::fs::write(&state.config_path, &original_file) {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "export write failed and original TOML restore also failed: {restore_error}"
                ),
            );
        }
        return response;
    }
    if let Err(e) = crate::db::accept_toml_baseline(&conn, &exported, &state.config_path) {
        return match std::fs::write(&state.config_path, &original_file) {
            Ok(()) => err(&state, StatusCode::INTERNAL_SERVER_ERROR, format!("record export baseline failed; original TOML restored: {e}")),
            Err(restore_error) => err(&state, StatusCode::INTERNAL_SERVER_ERROR, format!("record export baseline failed ({e}) and original TOML restore failed ({restore_error})")),
        };
    }
    Json(serde_json::json!({"exported":true,"fingerprint":crate::db::config_fingerprint(&state.config_path)})).into_response()
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

#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutLanAccessBody {
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct LanAccessResponse {
    pub enabled: bool,
    pub bind: String,
}

pub async fn get_lan_access(State(state): State<AppState>) -> Response {
    let enabled = state.auth.lan_access_enabled.load(Ordering::SeqCst);
    let bind = state
        .listen
        .get()
        .map(|handle| handle.bound().to_string())
        .unwrap_or_default();
    Json(LanAccessResponse { enabled, bind }).into_response()
}

pub async fn put_lan_access(State(state): State<AppState>, body: Bytes) -> Response {
    let request: PutLanAccessBody = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid LAN access body: {error}"),
            )
        }
    };
    if crate::auth::current_mode(&state) != crate::config::RunMode::Local {
        return err(
            &state,
            StatusCode::CONFLICT,
            "LAN access mode is only available in local mode",
        );
    }
    if std::env::var("JEV_BIND")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return err(&state, StatusCode::CONFLICT, "JEV_BIND overrides the application listen setting; clear it before changing LAN access");
    }
    let Some(handle) = state.listen.get() else {
        return err(
            &state,
            StatusCode::SERVICE_UNAVAILABLE,
            "listen supervisor unavailable",
        );
    };
    let current_enabled = state.auth.lan_access_enabled.load(Ordering::SeqCst);
    let current_addr = handle.bound();
    if current_enabled == request.enabled {
        return Json(LanAccessResponse {
            enabled: current_enabled,
            bind: current_addr.to_string(),
        })
        .into_response();
    }

    let mut config = match read_value(&state.config_path) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(table) = config.as_table_mut() else {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            "config root is not a table",
        );
    };

    let (target, explicit_after, previous_bind) = if request.enabled {
        let previous = table
            .get("bind")
            .and_then(toml::Value::as_str)
            .unwrap_or("auto")
            .to_owned();
        (
            std::net::SocketAddr::from(([0, 0, 0, 0], current_addr.port())),
            true,
            Some(previous),
        )
    } else {
        let previous = table
            .get("lan_previous_bind")
            .and_then(toml::Value::as_str)
            .unwrap_or("auto");
        if previous == "auto" {
            (handle.defaults().local, false, None)
        } else {
            match previous.parse::<std::net::SocketAddr>() {
                Ok(addr) => (addr, true, None),
                Err(error) => {
                    return err(
                        &state,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("invalid saved listen address: {error}"),
                    )
                }
            }
        }
    };

    if let Err(error) = handle.rebind(target).await {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("LAN listen rebind failed: {error}"),
        );
    }
    if request.enabled {
        table.insert(
            "lan_previous_bind".into(),
            toml::Value::String(previous_bind.unwrap_or_else(|| "auto".into())),
        );
        table.insert("bind".into(), toml::Value::String(target.to_string()));
        table.insert("lan_access_enabled".into(), toml::Value::Boolean(true));
    } else {
        table.remove("lan_previous_bind");
        table.insert("lan_access_enabled".into(), toml::Value::Boolean(false));
        if explicit_after {
            table.insert("bind".into(), toml::Value::String(target.to_string()));
        } else {
            table.remove("bind");
        }
    }
    if let Err(response) = write_value(&state.config_path, &config) {
        if let Err(restore_error) = handle.rebind(current_addr).await {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("save LAN setting failed and listen rollback failed: {restore_error}"),
            );
        }
        return response;
    }
    handle.set_explicit(explicit_after);
    state
        .auth
        .lan_access_enabled
        .store(request.enabled, Ordering::SeqCst);
    tracing::info!(enabled = request.enabled, bind = %target, "LAN access updated");
    Json(LanAccessResponse {
        enabled: request.enabled,
        bind: target.to_string(),
    })
    .into_response()
}

#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct PutGatewayBody {
    pub running: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct GatewayControlResponse {
    pub running: bool,
}

pub async fn get_gateway(State(state): State<AppState>) -> Json<GatewayControlResponse> {
    Json(GatewayControlResponse {
        running: state.gateway_enabled.load(Ordering::SeqCst),
    })
}

/// Suspend or resume new model calls while leaving the console and admin API available.
pub async fn put_gateway(State(state): State<AppState>, body: Bytes) -> Response {
    let request: PutGatewayBody = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid gateway state body: {error}"),
            )
        }
    };
    let mut config = match read_value(&state.config_path) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(table) = config.as_table_mut() else {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            "config root is not a table",
        );
    };
    table.insert(
        "gateway_enabled".into(),
        toml::Value::Boolean(request.running),
    );
    if let Err(response) = write_value(&state.config_path, &config) {
        return response;
    }
    state
        .gateway_enabled
        .store(request.running, Ordering::SeqCst);
    Json(GatewayControlResponse {
        running: request.running,
    })
    .into_response()
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
        uptime_s: PROCESS_START.get_or_init(Instant::now).elapsed().as_secs(),
    })
    .into_response()
}

/// `GET /v1/admin/telemetry` — process-local session telemetry for Dashboard.
pub async fn get_telemetry(
    State(state): State<AppState>,
) -> Json<crate::telemetry::TelemetrySnapshot> {
    Json(state.telemetry.snapshot())
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
#[allow(clippy::result_large_err)]
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
    *state.auth.admin_password.write().expect("password lock") = Some(new_password.to_string());
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
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid mode body: {e}"),
            )
        }
    };
    let new_mode = match crate::config::RunMode::parse(&req.mode) {
        Ok(m) => m,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, e.to_string()),
    };
    if let Some(pw) = &req.admin_password {
        if pw.trim().is_empty() {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                "admin_password must not be empty",
            );
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
                *state.auth.admin_password.write().expect("password lock") = Some(eff);
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
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid listen body: {e}"),
            )
        }
    };
    let Some(handle) = state.listen.get() else {
        return err(
            &state,
            StatusCode::SERVICE_UNAVAILABLE,
            "listen supervisor unavailable",
        );
    };

    // 目标地址：auto → 当前 mode 成对默认；否则解析显式值
    let (target, explicit_after) = if req.addr.trim().eq_ignore_ascii_case("auto") {
        (
            handle
                .defaults()
                .for_mode(crate::auth::current_mode(&state)),
            false,
        )
    } else {
        match req.addr.trim().parse::<std::net::SocketAddr>() {
            Ok(a) => (a, true),
            Err(_) => {
                return err(
                    &state,
                    StatusCode::BAD_REQUEST,
                    format!(
                        "invalid bind addr '{}' (expected ip:port, or \"auto\")",
                        req.addr
                    ),
                )
            }
        }
    };

    // Rebind（失败 → 旧监听未动 / 已恢复；不写文件）
    if let Err(e) = handle.rebind(target).await {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("rebind failed: {e}"),
        );
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
        reason: if explicit_after {
            None
        } else {
            Some("auto".into())
        },
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
    use crate::auth::{
        addr_allowed_in_local_mode, addr_is_loopback, bearer_token, current_mode, has_valid_session,
    };
    // in-handler 鉴权（password 不在 admin 中间件门内 —— login 同理）
    let loopback = addr_is_loopback(connect.as_ref().map(|axum::extract::ConnectInfo(a)| a));
    match current_mode(&state) {
        crate::config::RunMode::Local => {
            if !addr_allowed_in_local_mode(
                &state,
                connect
                    .as_ref()
                    .map(|axum::extract::ConnectInfo(addr)| addr),
            ) {
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
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid password body: {e}"),
            )
        }
    };
    if req.password.trim().is_empty() {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "password must not be empty",
        );
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

/// 从 provider 的同源模型目录获取可用模型。
///
/// TypeSafe 官方约定为 `GET /v1/models`，而 provider 配置保存的是完整
/// `POST /v1/systemone` 地址。使用 URL 解析器只替换最后一个 path segment，
/// 保留 host、scheme 和可能的 base path；绝不通过字符串拼接把 key 带入 URL。
pub async fn discover_provider_models(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let cfg = match load_config(&state) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(provider) = cfg.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider '{id}' not found"),
        );
    };

    let key = cfg.effective_api_key(&id);
    Json(discover_models(&provider.kind, &provider.base, key.as_deref()).await).into_response()
}

/// Discover models for a new provider form without persisting the draft.
pub async fn discover_provider_models_draft(
    State(_state): State<AppState>,
    Json(input): Json<ProviderModelsInput>,
) -> Response {
    let key = input
        .api_key
        .filter(|key| !key.trim().is_empty())
        .or_else(|| {
            input
                .api_key_env
                .as_deref()
                .and_then(|name| std::env::var(name).ok())
        });
    Json(discover_models(&input.kind, &input.base, key.as_deref()).await).into_response()
}

async fn discover_models(kind: &str, base: &str, api_key: Option<&str>) -> ProviderModelsResult {
    let models_url = match model_catalog_url(base) {
        Ok(url) => url,
        Err(error) => {
            return ProviderModelsResult {
                ok: false,
                latency_ms: 0,
                status: 0,
                models: Vec::new(),
                error: Some(redact(&error, &[])),
            };
        }
    };
    if !matches!(kind, "vercel" | "laya" | "typesafe" | "openrouter") {
        return ProviderModelsResult {
            ok: false,
            latency_ms: 0,
            status: 0,
            models: Vec::new(),
            error: Some(format!("unsupported provider kind '{kind}'")),
        };
    }
    let keys = api_key.map(str::to_string).into_iter().collect::<Vec<_>>();
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ProviderModelsResult {
                ok: false,
                latency_ms: 0,
                status: 0,
                models: Vec::new(),
                error: Some(redact(&error.to_string(), &keys)),
            };
        }
    };

    let mut request = client.get(models_url);
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }
    let started = Instant::now();
    let result = match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let success = response.status().is_success();
            match response.bytes().await {
                Ok(bytes) if success => {
                    match serde_json::from_slice::<serde_json::Value>(&bytes)
                        .ok()
                        .and_then(|body| extract_model_ids(&body))
                    {
                        Some(models) if !models.is_empty() => ProviderModelsResult {
                            ok: true,
                            latency_ms: started.elapsed().as_millis() as u64,
                            status,
                            models,
                            error: None,
                        },
                        _ => ProviderModelsResult {
                            ok: false,
                            latency_ms: started.elapsed().as_millis() as u64,
                            status,
                            models: Vec::new(),
                            error: Some("model catalog response contained no model IDs".into()),
                        },
                    }
                }
                Ok(bytes) => ProviderModelsResult {
                    ok: false,
                    latency_ms: started.elapsed().as_millis() as u64,
                    status,
                    models: Vec::new(),
                    error: Some(redact(
                        &format!(
                            "upstream returned HTTP {status}: {}",
                            String::from_utf8_lossy(&bytes)
                        ),
                        &keys,
                    )),
                },
                Err(error) => ProviderModelsResult {
                    ok: false,
                    latency_ms: started.elapsed().as_millis() as u64,
                    status,
                    models: Vec::new(),
                    error: Some(redact(&format!("read model catalog: {error}"), &keys)),
                },
            }
        }
        Err(error) => ProviderModelsResult {
            ok: false,
            latency_ms: started.elapsed().as_millis() as u64,
            status: 0,
            models: Vec::new(),
            error: Some(redact(&error.to_string(), &keys)),
        },
    };
    result
}

fn model_catalog_url(base: &str) -> Result<reqwest::Url, String> {
    let mut url =
        reqwest::Url::parse(base).map_err(|error| format!("invalid provider base URL: {error}"))?;
    let path = url.path().trim_end_matches('/');
    if path.ends_with("/chat/completions") {
        let prefix = path.trim_end_matches("/chat/completions");
        url.set_path(&format!("{prefix}/models"));
        return Ok(url);
    }
    let prefix = path
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
    url.set_path(&format!("{prefix}/models"));
    Ok(url)
}

fn extract_model_ids(body: &serde_json::Value) -> Option<Vec<String>> {
    let values = body
        .get("models")
        .or_else(|| body.get("data"))
        .and_then(serde_json::Value::as_array)?;
    let mut models = Vec::new();
    for value in values {
        let id = value
            .as_str()
            .or_else(|| value.get("id").and_then(serde_json::Value::as_str))
            .or_else(|| value.get("name").and_then(serde_json::Value::as_str));
        if let Some(id) = id.filter(|id| !id.trim().is_empty()) {
            if !models.iter().any(|existing| existing == id) {
                models.push(id.to_string());
            }
        }
    }
    Some(models)
}

/// Direct upstream invocation for comparison/testing. The path selects an existing
/// daemon-side provider config; the request never carries a provider URL or credential.
pub async fn invoke_provider(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    body: Bytes,
) -> Response {
    if !state.gateway_enabled.load(Ordering::SeqCst) {
        return err(
            &state,
            StatusCode::SERVICE_UNAVAILABLE,
            "gateway service is stopped",
        );
    }
    let req: jev_protocol::JevRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid JevRequest: {e}"),
            )
        }
    };
    let config = match load_config(&state) {
        Ok(config) => config,
        Err(response) => return response,
    };
    let Some(provider) = config.providers.get(&id) else {
        return err(
            &state,
            StatusCode::NOT_FOUND,
            format!("provider not found: {id}"),
        );
    };
    if !provider.enabled {
        return err(
            &state,
            StatusCode::FORBIDDEN,
            format!("provider is disabled: {id}"),
        );
    }
    if !provider.models.is_empty() && !provider.models.iter().any(|model| model == &req.model) {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "requested model is not listed for this provider",
        );
    }
    let Some(upstream) = state.registry.router().upstream(&id) else {
        return err(
            &state,
            StatusCode::SERVICE_UNAVAILABLE,
            format!("provider adapter is unavailable: {id}"),
        );
    };
    for question in req.questions.values() {
        if !upstream.capabilities().supports(question.question_type()) {
            return err(
                &state,
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "provider '{}' does not support question type '{}'",
                    id,
                    question.question_type().as_str()
                ),
            );
        }
    }
    let selected_model = req.model.clone();
    let started = Instant::now();
    let mut result = upstream.evaluate(req).await;
    let elapsed = started.elapsed();
    let request_id =
        record_direct_provider_call(&state, &id, &selected_model, &mut result, elapsed);
    let mut response = match result {
        Ok(response) => Json(response).into_response(),
        Err(error) => {
            let status =
                StatusCode::from_u16(error.http_status()).unwrap_or(StatusCode::BAD_GATEWAY);
            let keys = known_keys_snapshot(&state);
            error_response(
                status,
                error.to_string(),
                Some(id),
                error.retryable(),
                &keys,
            )
        }
    };
    if let Some(request_id) = request_id {
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response.headers_mut().insert("x-jev-request-id", value);
        }
    }
    response
}

/// Persist a direct comparison attempt with the same durable cursor and safe metadata
/// used by public-entry calls. Request and response bodies are deliberately omitted.
fn record_direct_provider_call(
    state: &AppState,
    provider_id: &str,
    upstream_model: &str,
    result: &mut Result<jev_protocol::JevResponse, jev_core::upstream::JevError>,
    elapsed: Duration,
) -> Option<String> {
    let (success, status, cost_usd, upstream_calls, usage) = match result {
        Ok(response) => (
            true,
            200_u16,
            response.cost_usd,
            response.upstream_calls.unwrap_or(1),
            response.usage.clone(),
        ),
        Err(error) => (false, error.http_status(), None, 1, None),
    };
    let latency_ms = elapsed.as_millis().min(i64::MAX as u128) as i64;
    let endpoint_id = format!("direct:{provider_id}:{upstream_model}");
    let route_key = format!("{provider_id}→{upstream_model}");
    let mut route_trace = serde_json::json!({
        "kind": "direct_upstream",
        "provider_config_id": provider_id,
        "selected_provider": provider_id,
        "selected_model": upstream_model,
        "selected_hops": [provider_id, upstream_model],
        "strategy": "direct",
        "upstream_calls": upstream_calls,
        "gateway_latency_ms": latency_ms,
    });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let usage_json = usage
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok());

    let Ok(mut conn) = state.db_conn.lock() else {
        tracing::warn!("failed to lock direct-call history database");
        return None;
    };
    let Ok(tx) = conn.transaction() else {
        tracing::warn!("failed to begin direct-call history transaction");
        return None;
    };
    if let Err(error) = tx.execute(
        "INSERT INTO call_logs (timestamp, endpoint_id, route_key, upstream_provider, upstream_model, success, latency_ms, error_message, token_id, cost_usd, upstream_calls, usage_json, http_status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)",
        rusqlite::params![
            now,
            endpoint_id,
            route_key,
            provider_id,
            upstream_model,
            if success { 1 } else { 0 },
            latency_ms,
            if success { None::<String> } else { Some(format!("HTTP {status}")) },
            cost_usd,
            i64::from(upstream_calls),
            usage_json,
            i64::from(status),
        ],
    ) {
        tracing::warn!(error = %error, "failed to persist direct-call statistics");
        return None;
    }
    let log_id = tx.last_insert_rowid();
    let request_id = format!("jev-{log_id}");
    if let Some(trace) = route_trace.as_object_mut() {
        trace.insert(
            "request_id".into(),
            serde_json::Value::String(request_id.clone()),
        );
    }
    let route_trace_json = serde_json::to_string(&route_trace).ok();
    if let Err(error) = tx.execute(
        "UPDATE call_logs SET request_id=?, route_trace_json=? WHERE id=?",
        rusqlite::params![request_id, route_trace_json, log_id],
    ) {
        tracing::warn!(error = %error, "failed to persist direct-call trace");
        return None;
    }
    if let Err(error) = tx.commit() {
        tracing::warn!(error = %error, "failed to commit direct-call history");
        return None;
    }

    if let Ok(response) = result {
        response.extra.insert(
            "request_id".into(),
            serde_json::Value::String(request_id.clone()),
        );
        response
            .extra
            .insert("route_trace".into(), route_trace.clone());
    }
    let detail = serde_json::json!({
        "endpoint_id": endpoint_id,
        "success": success,
        "status": status,
        "provider": provider_id,
        "upstream_model": upstream_model,
        "latency_ms": latency_ms,
        "upstream_calls": upstream_calls,
        "usage": usage,
        "cost_usd": cost_usd,
        "request_id": request_id,
        "route_trace": route_trace,
    })
    .to_string();
    state
        .events
        .push_for_token_with_id(log_id as u64, "request", detail, None);
    Some(request_id)
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

    #[test]
    fn model_catalog_url_replaces_only_the_systemone_segment() {
        assert_eq!(
            model_catalog_url("https://api.typesafe.ai/v1/systemone")
                .unwrap()
                .as_str(),
            "https://api.typesafe.ai/v1/models"
        );
        assert_eq!(
            model_catalog_url("http://127.0.0.1:8783/v1/systemone")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:8783/v1/models"
        );
        assert_eq!(
            model_catalog_url("https://openrouter.ai/api/v1/chat/completions")
                .unwrap()
                .as_str(),
            "https://openrouter.ai/api/v1/models"
        );
    }

    #[test]
    fn model_catalog_parser_accepts_typesafe_and_openai_shapes_and_deduplicates() {
        let typesafe = serde_json::json!({"models": [{"id": "jev-latest"}, {"name": "jev-1.13.0"}, "jev-latest"]});
        assert_eq!(
            extract_model_ids(&typesafe).unwrap(),
            vec!["jev-latest", "jev-1.13.0"]
        );
        let openai = serde_json::json!({"data": [{"id": "model-a"}, {"id": "model-b"}]});
        assert_eq!(
            extract_model_ids(&openai).unwrap(),
            vec!["model-a", "model-b"]
        );
        assert!(extract_model_ids(&serde_json::json!({"models": []}))
            .unwrap()
            .is_empty());
    }

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

    async fn send(
        app: axum::Router,
        method: &str,
        uri: &str,
        body: Option<String>,
    ) -> (u16, String) {
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

        // 结构级：ProviderView 带账号/模型元数据，但无 api_key
        let doc: serde_json::Value = serde_json::from_str(&body).unwrap();
        let vercel = doc["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "vercel")
            .expect("vercel in list");
        let obj = vercel.as_object().unwrap();
        assert_eq!(obj.len(), 11);
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

    #[tokio::test]
    async fn json_backup_exports_plaintext_keys_only_after_explicit_confirmation() {
        let path = temp_config("json-backup-export", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let request = |body: String| {
            Request::builder()
                .method("POST")
                .uri("/v1/admin/config/export-json")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap()
        };

        let (unconfirmed_status, unconfirmed_body) = send(
            app.clone(),
            "POST",
            "/v1/admin/config/export-json",
            Some(r#"{"confirm":false}"#.into()),
        )
        .await;
        assert_eq!(unconfirmed_status, 400, "{unconfirmed_body}");
        assert!(!unconfirmed_body.contains(FAKE_KEY));

        let response = app
            .oneshot(request(r#"{"confirm":true}"#.into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CACHE_CONTROL],
            "no-store, private"
        );
        assert_eq!(response.headers()[axum::http::header::PRAGMA], "no-cache");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            text.contains(FAKE_KEY),
            "explicit backup must preserve a usable key"
        );
        assert!(!text.contains("admin_password"));
        assert!(!text.contains("auth_tokens"));
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["schema_version"], RUNTIME_BACKUP_SCHEMA_VERSION);
        assert_eq!(json["providers"]["vercel"]["api_key"], FAKE_KEY);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn json_backup_import_validates_before_replacing_runtime_snapshot() {
        let path = temp_config("json-backup-import", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let invalid = serde_json::json!({
            "confirm": true,
            "backup": {
                "schema_version": 1,
                "providers": {"vercel": {"kind":"vercel", "base":"https://example.invalid", "models":[], "api_key":"replacement-secret", "enabled":true}},
                "routes": [{"left":"jev", "right":"missing", "match":"exact", "priority":10, "sticky":"session", "on_error":"next"}]
            }
        });
        let (invalid_status, invalid_body) = send(
            app.clone(),
            "POST",
            "/v1/admin/config/import-json",
            Some(invalid.to_string()),
        )
        .await;
        assert_eq!(invalid_status, 400, "{invalid_body}");
        assert!(!invalid_body.contains("replacement-secret"));
        let (before_status, before) = send(app.clone(), "GET", "/v1/admin/providers", None).await;
        assert_eq!(before_status, 200);
        assert!(
            before.contains("laya"),
            "rejected import must leave active providers unchanged"
        );

        let valid = serde_json::json!({
            "confirm": true,
            "backup": {
                "schema_version": 1,
                "providers": {"vercel": {"kind":"vercel", "base":"https://example.invalid/v4/eval", "models":[], "api_key":"replacement-secret", "enabled":true}},
                "routes": [{"left":"jev", "right":"vercel", "match":"exact", "priority":10, "sticky":"session", "on_error":"next"}]
            }
        });
        let (status, body) = send(
            app.clone(),
            "POST",
            "/v1/admin/config/import-json",
            Some(valid.to_string()),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert!(
            !body.contains("no-store"),
            "the JSON response itself should not echo backup content"
        );
        let (provider_status, provider_body) =
            send(app.clone(), "GET", "/v1/admin/providers", None).await;
        assert_eq!(provider_status, 200);
        assert!(!provider_body.contains("replacement-secret"));
        assert!(!provider_body.contains("sk-test1234abcd"));
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

    /* ── 原子 route transaction 成环 → 400（文案含 环/cycle） ──── */

    #[tokio::test]
    async fn put_routes_cycle_is_400_with_cycle_message() {
        let path = temp_config("cycle", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());
        let before = crate::db::load_runtime_snapshot(&state.db_conn.lock().unwrap())
            .unwrap()
            .unwrap()
            .routes;

        let body = r#"{"operations":[
            {"op":"create","route":{"left":"a","right":"b","priority":1}},
            {"op":"create","route":{"left":"b","right":"a","priority":2}}
        ]}"#;
        let (status, resp) = send(
            app,
            "POST",
            "/v1/admin/routes/transaction",
            Some(body.into()),
        )
        .await;
        assert_eq!(status, 400, "resp={resp}");
        assert!(
            resp.contains("环") || resp.contains("cycle"),
            "文案须含 环/cycle: {resp}"
        );

        // 文件未被污染（检环在落盘前）
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            !on_disk.contains("left = \"a\""),
            "环配置不得落盘: {on_disk}"
        );
        let after = crate::db::load_runtime_snapshot(&state.db_conn.lock().unwrap())
            .unwrap()
            .unwrap()
            .routes;
        assert_eq!(after, before, "失败事务不得改变 SQLite runtime snapshot");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn legacy_routes_put_is_not_exposed() {
        let path = temp_config("legacy-routes-put", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());
        let (status, body) = send(
            app,
            "PUT",
            "/v1/admin/routes",
            Some(r#"{"routes":[]}"#.into()),
        )
        .await;
        assert_eq!(status, 405, "整表 routes PUT 必须被移除: {body}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 原子 route transaction 成功 → 运行时 Router 热更 ──────── */

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

        let body = r#"{"operations":[
            {"op":"delete","edge_id":"jev=>vercel"},
            {"op":"create","route":{"left":"new-model","right":"vercel","upstream_model":"typesafe-ai/jev","priority":7}}
        ]}"#;
        let (status, resp) = send(
            app,
            "POST",
            "/v1/admin/routes/transaction",
            Some(body.into()),
        )
        .await;
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
        assert!(
            !on_disk.contains("new-model"),
            "routine route writes must stay in the SQLite runtime snapshot: {on_disk}"
        );
        let storage = crate::db::load_runtime_snapshot(&state.db_conn.lock().unwrap())
            .unwrap()
            .unwrap();
        assert!(storage.routes.iter().any(|edge| edge.left == "new-model"));
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

    #[tokio::test]
    async fn model_discovery_fetches_catalog_with_provider_key_and_deduplicates() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .and(wiremock::matchers::header(
                "authorization",
                "Bearer sk-test1234abcd",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "models": [{"id": "jev-latest"}, {"name": "jev-1.13.0"}, "jev-latest"]
                })),
            )
            .expect(1)
            .mount(&server)
            .await;

        let cfg = format!(
            r#"
[providers.typesafe]
kind = "typesafe"
base = "{}/v1/systemone"
api_key = "{}"
enabled = true
"#,
            server.uri(),
            FAKE_KEY
        );
        let path = temp_config("models-discovery", &cfg);
        let (app, _state) = app_at(path.clone());
        let (status, body) = send(
            app,
            "POST",
            "/v1/admin/providers/typesafe/models",
            Some("{}".into()),
        )
        .await;

        assert_eq!(status, 200, "{body}");
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["ok"], true, "{body}");
        assert_eq!(value["status"], 200, "{body}");
        assert_eq!(
            value["models"],
            serde_json::json!(["jev-latest", "jev-1.13.0"])
        );
        assert!(value["latency_ms"].is_number());
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
                                    .any(|l| {
                                        l.starts_with("api_key:") || l.starts_with("pub api_key:")
                                    });
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
        assert!(
            scanned >= 8,
            "至少应扫到 8 个 Serialize struct，实际 {scanned}"
        );
    }

    /* ── PUT providers：省略 api_key = 保留原 key（H3 备注①） ──── */

    #[tokio::test]
    async fn put_providers_omitted_api_key_preserves_existing() {
        let path = temp_config("put-keep", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());

        // 只翻 enabled、不带 api_key → 原 key 保留
        let body = r#"{"id":"vercel","kind":"vercel","base":"https://example.invalid/v4/eval","enabled":false}"#;
        let (status, resp) =
            send(app, "PUT", "/v1/admin/providers/vercel", Some(body.into())).await;
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

        // SQLite runtime snapshot is authoritative; legacy TOML stays untouched until explicit export.
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            on_disk.contains(FAKE_KEY),
            "省略 api_key 应保留原 key: {on_disk}"
        );
        assert!(
            on_disk.contains("enabled = true"),
            "routine provider writes must not mutate legacy TOML: {on_disk}"
        );
        let snapshot = crate::db::load_runtime_snapshot(&state.db_conn.lock().unwrap())
            .unwrap()
            .unwrap();
        let vercel_cfg = snapshot.providers.get("vercel").unwrap();
        assert_eq!(vercel_cfg.api_key.as_deref(), Some(FAKE_KEY));
        assert!(!vercel_cfg.enabled);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn put_providers_deleting_provider_cascades_routes_and_refreshes_router() {
        let path = temp_config("put-delete-provider", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());

        let graph = r#"{"operations":[
            {"op":"delete","edge_id":"jev=>vercel"},
            {"op":"delete","edge_id":"laya-english=>laya"},
            {"op":"create","route":{"left":"jev","right":"fallback","match":"exact","priority":10}},
            {"op":"create","route":{"left":"fallback","right":"vercel","match":"exact","priority":10}},
            {"op":"create","route":{"left":"fallback","right":"laya","match":"exact","priority":20}},
            {"op":"create","route":{"left":"unrelated","right":"laya","match":"exact","priority":10}},
            {"op":"create","route":{"left":"vercel","right":"laya","match":"exact","priority":5}},
            {"op":"create","route":{"left":"jev2","right":"vercel","match":"exact","priority":5}}
        ]}"#;
        let (route_status, route_response) = send(
            app.clone(),
            "POST",
            "/v1/admin/routes/transaction",
            Some(graph.into()),
        )
        .await;
        assert_eq!(route_status, 200, "{route_response}");

        let (status, response) =
            send(app.clone(), "DELETE", "/v1/admin/providers/vercel", None).await;
        assert_eq!(status, 200, "{response}");

        let (routes_status, routes_body) = send(app.clone(), "GET", "/v1/admin/routes", None).await;
        assert_eq!(routes_status, 200, "{routes_body}");
        let routes: serde_json::Value = serde_json::from_str(&routes_body).unwrap();
        let route_list = routes["routes"].as_array().unwrap();
        assert_eq!(route_list.len(), 3);
        assert!(route_list
            .iter()
            .any(|edge| edge["left"] == "jev" && edge["right"] == "fallback"));
        assert!(route_list
            .iter()
            .any(|edge| edge["left"] == "fallback" && edge["right"] == "laya"));
        assert!(route_list
            .iter()
            .any(|edge| edge["left"] == "unrelated" && edge["right"] == "laya"));
        assert!(!route_list.iter().any(|edge| edge["right"] == "vercel"));
        assert!(!route_list
            .iter()
            .any(|edge| edge["left"] == "vercel" || edge["left"] == "jev2"));

        let snapshot = crate::db::load_runtime_snapshot(&state.db_conn.lock().unwrap())
            .unwrap()
            .unwrap();
        assert!(!snapshot.providers.contains_key("vercel"));
        assert_eq!(snapshot.routes.len(), 3);
        assert_eq!(state.registry.router().edges().len(), 3);
        let plan = state
            .registry
            .router()
            .plan("jev", &jev_core::router::RouteCtx::default())
            .unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].candidate.upstream_id, "laya");

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn get_graph_projects_compatible_snapshot_as_typed_nodes() {
        let path = temp_config("get-graph", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let (status, body) = send(app, "GET", "/v1/admin/graph", None).await;
        assert_eq!(status, 200, "{body}");
        let graph: serde_json::Value = serde_json::from_str(&body).unwrap();
        let nodes = graph["nodes"].as_array().unwrap();
        assert!(nodes
            .iter()
            .any(|node| node["id"] == "jev" && node["kind"] == "public"));
        assert!(nodes
            .iter()
            .any(|node| node["id"] == "vercel" && node["kind"] == "provider"));
        assert!(nodes.iter().all(|node| node.get("api_key").is_none()));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn capabilities_discovery_exposes_lifecycle_scopes_without_secrets() {
        let path = temp_config("capabilities", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let (status, body) = send(app, "GET", "/v1/admin/capabilities", None).await;
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("providers.lifecycle.controllable"));
        assert!(body.contains("provider:lifecycle:execute"));
        assert!(body.contains("host_commands:enable"));
        assert!(!body.contains("sk-test1234abcd"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn host_command_gate_round_trips_without_changing_provider_routes() {
        let path = temp_config("host-commands", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());
        let (get_status, before) = send(app.clone(), "GET", "/v1/admin/host-commands", None).await;
        assert_eq!(get_status, 200, "{before}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&before).unwrap()["enabled"],
            false
        );

        let (put_status, body) = send(
            app.clone(),
            "PUT",
            "/v1/admin/host-commands",
            Some(r#"{"enabled":true}"#.into()),
        )
        .await;
        assert_eq!(put_status, 200, "{body}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap()["enabled"],
            true
        );

        let (status, _) = send(app, "GET", "/v1/admin/host-commands", None).await;
        assert_eq!(status, 200);
        let snapshot = crate::db::load_runtime_snapshot(&state.db_conn.lock().unwrap())
            .unwrap()
            .unwrap();
        assert!(snapshot.providers.contains_key("vercel"));
        assert!(snapshot.allow_host_commands);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn provider_lifecycle_summary_round_trips_without_exposing_commands() {
        let path = temp_config("lifecycle-summary", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let body = r#"{"id":"vercel","kind":"vercel","base":"https://example.invalid/v4/eval","enabled":true,
             "lifecycle":{"controllable":true,"process_policy":"persistent","mode":"startup_check",
               "program":"C:/Tools/OpenJev/openjev.exe","args":["--port","11436"]}}"#;
        let (put_status, _) = send(
            app.clone(),
            "PUT",
            "/v1/admin/providers/vercel",
            Some(body.into()),
        )
        .await;
        assert_eq!(put_status, 200);
        let (get_status, response) = send(app, "GET", "/v1/admin/providers", None).await;
        assert_eq!(get_status, 200);
        assert!(response.contains("\"controllable\":true"));
        assert!(response.contains("\"service_state\":\"unknown\""));
        assert!(
            !response.contains("openjev.exe"),
            "command body must not be returned in the summary"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn lifecycle_domain_endpoint_updates_config_without_echoing_command_body() {
        let path = temp_config("lifecycle-domain", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let lifecycle = r#"{
            "controllable":true,
            "process_policy":"persistent",
            "mode":"on_demand",
            "program":"C:/Tools/OpenJev/openjev.exe",
            "args":["--port","11436"]
        }"#;
        let (put_status, put_body) = send(
            app.clone(),
            "PUT",
            "/v1/admin/providers/vercel/lifecycle",
            Some(lifecycle.into()),
        )
        .await;
        assert_eq!(put_status, 200, "{put_body}");
        assert!(put_body.contains("openjev.exe"));

        let (get_status, get_body) =
            send(app, "GET", "/v1/admin/providers/vercel/lifecycle", None).await;
        assert_eq!(get_status, 200, "{get_body}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&get_body).unwrap()["config"]["mode"],
            "on_demand"
        );
        assert!(get_body.contains("openjev.exe"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn lifecycle_errors_expose_machine_code_detail_and_remediation() {
        let path = temp_config("lifecycle-error", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let invalid = r#"{
            "controllable":true,
            "process_policy":"persistent",
            "mode":"automatic",
            "args":[]
        }"#;
        let (status, body) = send(
            app,
            "PUT",
            "/v1/admin/providers/vercel/lifecycle",
            Some(invalid.into()),
        )
        .await;
        assert_eq!(status, 400, "{body}");
        let error: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(error["code"], "invalid_lifecycle_configuration");
        assert_eq!(
            error["detail"],
            "mode must be manual, startup_check, or on_demand"
        );
        assert!(error["remediation"].as_str().unwrap().contains("save"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn put_providers_rejects_duplicate_and_empty_fields() {
        let path = temp_config("put-bad", CFG_WITH_KEY);
        let (app, state) = app_at(path.clone());

        let dup = r#"{"id":"a","kind":"k","base":"b","enabled":true}"#;
        let (status, resp) = send(app, "PUT", "/v1/admin/providers/a", Some(dup.into())).await;
        assert_eq!(status, 400, "{resp}");

        let unsupported =
            r#"{"id":"a","kind":"vercel-gateway","base":"https://example.invalid","enabled":true}"#;
        let (status, resp) = send(
            build_app(state.clone()),
            "PUT",
            "/v1/admin/providers/a",
            Some(unsupported.into()),
        )
        .await;
        assert_eq!(status, 400, "{resp}");
        assert!(resp.contains("vercel"), "{resp}");

        let empty_id = r#"{"id":"","kind":"k","base":"b","enabled":true}"#;
        let (status, resp) = send(
            build_app(state.clone()),
            "PUT",
            "/v1/admin/providers/a",
            Some(empty_id.into()),
        )
        .await;
        assert_eq!(status, 400, "{resp}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[tokio::test]
    async fn provider_collection_put_is_not_a_public_batch_write_endpoint() {
        let path = temp_config("provider-batch-disabled", CFG_WITH_KEY);
        let (app, _) = app_at(path.clone());
        let (status, body) = send(
            app,
            "PUT",
            "/v1/admin/providers",
            Some(r#"{"providers":[]}"#.into()),
        )
        .await;
        assert_eq!(status, 405, "{body}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── PUT routes right 引用校验 ──────────────────────────────── */

    #[tokio::test]
    async fn put_routes_rejects_unknown_right_node() {
        let path = temp_config("right", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        // right 既非 provider（vercel/laya）也非新表 exact left → 400
        let body = r#"{"operations":[{"op":"create","route":{"left":"jev","right":"ghost","priority":1}}]}"#;
        let (status, resp) = send(
            app,
            "POST",
            "/v1/admin/routes/transaction",
            Some(body.into()),
        )
        .await;
        assert_eq!(status, 400, "{resp}");
        assert!(resp.contains("ghost"), "{resp}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /* ── 别名 right 合法（新表 exact left 即别名节点） ──────────── */

    #[tokio::test]
    async fn put_routes_accepts_alias_right_within_payload() {
        let path = temp_config("alias", CFG_WITH_KEY);
        let (app, _state) = app_at(path.clone());

        let body = r#"{"operations":[
            {"op":"create","route":{"left":"jev","right":"jev-fast","priority":5}},
            {"op":"create","route":{"left":"jev-fast","right":"vercel","priority":10}}
        ]}"#;
        let (status, resp) = send(
            app,
            "POST",
            "/v1/admin/routes/transaction",
            Some(body.into()),
        )
        .await;
        assert_eq!(status, 200, "{resp}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
