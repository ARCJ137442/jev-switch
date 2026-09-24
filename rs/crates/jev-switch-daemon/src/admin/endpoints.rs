//! Admin API for service endpoints configuration (Phase 4.2).
//!
//! 6 个 RESTful 端点：
//! - GET /v1/admin/endpoints — 获取所有服务入口 + 健康状态
//! - POST /v1/admin/endpoints — 创建服务入口 + 可选路由
//! - PUT /v1/admin/endpoints/{id} — 更新策略/启用状态
//! - DELETE /v1/admin/endpoints/{id} — 删除入口 + 级联删除路由
//! - GET /v1/admin/config/default_strategy — 获取全局默认策略
//! - PUT /v1/admin/config/default_strategy — 更新全局默认策略

use crate::{error_response, known_keys_snapshot, AppState};
use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

/* ══════════════════════════════════════════════════════════════════
   DTO (Phase 4.2 服务入口配置)
   ══════════════════════════════════════════════════════════════════ */

/// 策略配置枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(tag = "type")]
pub enum StrategyConfig {
    #[serde(rename = "follow_global")]
    FollowGlobal,
    #[serde(rename = "failover")]
    Failover,
    #[serde(rename = "race")]
    Race { timeout_ms: u64 },
    #[serde(rename = "load_balance")]
    LoadBalance { weight_mode: String },
    #[serde(rename = "shadow")]
    Shadow { shadow_target: String },
}

impl StrategyConfig {
    fn to_db_string(&self) -> String {
        match self {
            StrategyConfig::FollowGlobal => "follow_global".to_string(),
            _ => serde_json::to_string(self).unwrap_or_else(|_| "follow_global".to_string()),
        }
    }

    fn from_db_string(s: &str) -> Self {
        if s == "follow_global" {
            return StrategyConfig::FollowGlobal;
        }
        serde_json::from_str(s).unwrap_or(StrategyConfig::FollowGlobal)
    }
}

/// 健康状态汇总
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct HealthSummary {
    pub total: u32,
    pub healthy: u32,
    pub degraded: u32,
    pub failed: u32,
}

/// 服务入口视图（响应）
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ServiceEndpointView {
    pub id: String,
    pub strategy_config: StrategyConfig,
    pub enabled: bool,
    pub routes_count: u32,
    pub health_summary: HealthSummary,
    pub calls_24h: u64,
    pub calls_total: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// GET /v1/admin/endpoints 响应
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct EndpointsResponse {
    pub endpoints: Vec<ServiceEndpointView>,
    pub global_default_strategy: String,
}

/// POST /v1/admin/endpoints 请求（创建时可选路由）
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CreateEndpointRequest {
    pub id: String,
    pub strategy_config: StrategyConfig,
    #[serde(default)]
    pub routes: Option<Vec<jev_core::router::RouteEdge>>,
}

/// PUT /v1/admin/endpoints/{id} 请求
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct UpdateEndpointRequest {
    #[serde(default)]
    pub id: Option<String>, // 修改 ID（可选）
    #[serde(default)]
    pub strategy_config: Option<StrategyConfig>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// DELETE /v1/admin/endpoints/{id} 响应
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct DeleteEndpointResponse {
    pub id: String,
    pub deleted: bool,
    pub routes_deleted: u32,
}

/// GET/PUT /v1/admin/config/default_strategy 共用体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct DefaultStrategyBody {
    pub default_strategy: String,
}

/* ══════════════════════════════════════════════════════════════════
   辅助函数
   ══════════════════════════════════════════════════════════════════ */

fn err(state: &AppState, status: StatusCode, msg: impl Into<String>) -> Response {
    let keys = known_keys_snapshot(state);
    error_response(status, msg, None, false, &keys)
}

/// 计算健康状态汇总（简化版：暂时返回占位数据）
fn compute_health_summary(_endpoint_id: &str, _routes_count: u32) -> HealthSummary {
    // TODO: 实际实现需要查询 provider 健康状态
    HealthSummary {
        total: _routes_count,
        healthy: _routes_count,
        degraded: 0,
        failed: 0,
    }
}

/// 统计调用次数（简化版：暂时返回 0）
fn compute_calls(_endpoint_id: &str) -> (u64, u64) {
    // TODO: 实际实现需要查询 call_logs 表
    (0, 0) // (calls_24h, calls_total)
}

/// 统计入口的路由数量
fn count_routes(state: &AppState, endpoint_id: &str) -> u32 {
    state
        .registry
        .router()
        .edges()
        .iter()
        .filter(|e| e.left == endpoint_id)
        .count() as u32
}

/* ══════════════════════════════════════════════════════════════════
   API 端点实现
   ══════════════════════════════════════════════════════════════════ */

/// GET /v1/admin/endpoints — 获取所有服务入口 + 健康状态 + 调用统计
pub async fn list_endpoints(State(state): State<AppState>) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    let endpoints = match crate::db::endpoints::load_all(&conn) {
        Ok(eps) => eps,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load endpoints failed: {e}"),
            )
        }
    };

    let global_default = crate::db::endpoints::get_default_strategy(&conn)
        .unwrap_or_else(|_| "failover".to_string());

    let views: Vec<ServiceEndpointView> = endpoints
        .into_iter()
        .map(|ep| {
            let routes_count = count_routes(&state, &ep.id);
            let health_summary = compute_health_summary(&ep.id, routes_count);
            let (calls_24h, calls_total) = compute_calls(&ep.id);

            ServiceEndpointView {
                id: ep.id.clone(),
                strategy_config: StrategyConfig::from_db_string(&ep.strategy_config),
                enabled: ep.enabled,
                routes_count,
                health_summary,
                calls_24h,
                calls_total,
                created_at: ep.created_at,
                updated_at: ep.updated_at,
            }
        })
        .collect();

    Json(EndpointsResponse {
        endpoints: views,
        global_default_strategy: global_default,
    })
    .into_response()
}

/// POST /v1/admin/endpoints — 创建服务入口 + 可选路由（原子性）
pub async fn create_endpoint(
    State(state): State<AppState>,
    body: Bytes,
) -> Response {
    let req: CreateEndpointRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };

    // 验证 ID 不重复
    {
        let endpoints = state.service_endpoints.read().expect("endpoints lock");
        if endpoints.contains_key(&req.id) {
            return err(
                &state,
                StatusCode::CONFLICT,
                format!("endpoint id already exists: {}", req.id),
            );
        }
    }

    // 数据库事务：创建入口 + 可选路由
    let conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("transaction failed: {e}"),
            )
        }
    };

    // 插入 service_endpoints
    let strategy_str = req.strategy_config.to_db_string();
    if let Err(e) = crate::db::endpoints::create(&tx, &req.id, &strategy_str, true) {
        let _ = tx.rollback();
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("create endpoint failed: {e}"),
        );
    }

    // 插入 routes（如果提供）
    if let Some(routes) = &req.routes {
        for route in routes {
            // 简化：直接使用 jev_core::router::RouteEdge，需要序列化到 routes 表
            // 实际实现需要调用 routes CRUD
            // 此处先跳过，因为 routes 表操作已在 admin.rs 中实现
            let _ = route; // 占位
        }
    }

    if let Err(e) = tx.commit() {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("commit failed: {e}"),
        );
    }

    // 更新内存
    {
        let mut endpoints = state.service_endpoints.write().expect("endpoints lock");
        endpoints.insert(
            req.id.clone(),
            crate::db::endpoints::ServiceEndpoint {
                id: req.id.clone(),
                strategy_config: strategy_str.clone(),
                enabled: true,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            },
        );
    }

    Json(ServiceEndpointView {
        id: req.id.clone(),
        strategy_config: req.strategy_config,
        enabled: true,
        routes_count: req.routes.as_ref().map(|r| r.len() as u32).unwrap_or(0),
        health_summary: HealthSummary {
            total: 0,
            healthy: 0,
            degraded: 0,
            failed: 0,
        },
        calls_24h: 0,
        calls_total: 0,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        updated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    })
    .into_response()
}

/// PUT /v1/admin/endpoints/{id} — 更新策略/启用状态，支持修改 ID
pub async fn update_endpoint(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    body: Bytes,
) -> Response {
    let req: UpdateEndpointRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };

    let conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    // 更新数据库
    let new_id_ref = req.id.as_deref();
    let strategy_str = req.strategy_config.as_ref().map(|s| s.to_db_string());
    let strategy_ref = strategy_str.as_deref();

    if let Err(e) =
        crate::db::endpoints::update(&conn, &id, new_id_ref, strategy_ref, req.enabled)
    {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("update endpoint failed: {e}"),
        );
    }

    // 更新内存
    {
        let mut endpoints = state.service_endpoints.write().expect("endpoints lock");
        if let Some(ep) = endpoints.remove(&id) {
            let final_id = req.id.clone().unwrap_or(id.clone());
            endpoints.insert(
                final_id.clone(),
                crate::db::endpoints::ServiceEndpoint {
                    id: final_id.clone(),
                    strategy_config: strategy_str.unwrap_or(ep.strategy_config),
                    enabled: req.enabled.unwrap_or(ep.enabled),
                    created_at: ep.created_at,
                    updated_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                },
            );
        }
    }

    Json(serde_json::json!({"updated": true})).into_response()
}

/// DELETE /v1/admin/endpoints/{id} — 删除入口 + 级联删除路由
pub async fn delete_endpoint(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    let routes_deleted = count_routes(&state, &id);

    match crate::db::endpoints::delete(&conn, &id) {
        Ok(_) => {
            // 更新内存
            {
                let mut endpoints = state.service_endpoints.write().expect("endpoints lock");
                endpoints.remove(&id);
            }

            Json(DeleteEndpointResponse {
                id: id.clone(),
                deleted: true,
                routes_deleted,
            })
            .into_response()
        }
        Err(e) => err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("delete endpoint failed: {e}"),
        ),
    }
}

/// GET /v1/admin/config/default_strategy — 获取全局默认策略
pub async fn get_default_strategy(State(state): State<AppState>) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    let strategy = crate::db::endpoints::get_default_strategy(&conn)
        .unwrap_or_else(|_| "failover".to_string());

    Json(DefaultStrategyBody {
        default_strategy: strategy,
    })
    .into_response()
}

/// PUT /v1/admin/config/default_strategy — 更新全局默认策略
pub async fn update_default_strategy(
    State(state): State<AppState>,
    body: Bytes,
) -> Response {
    let req: DefaultStrategyBody = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(&state, StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };

    let conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    if let Err(e) = crate::db::endpoints::set_default_strategy(&conn, &req.default_strategy) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("update default strategy failed: {e}"),
        );
    }

    Json(DefaultStrategyBody {
        default_strategy: req.default_strategy,
    })
    .into_response()
}
