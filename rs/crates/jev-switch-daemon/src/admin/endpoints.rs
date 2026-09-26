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
use std::collections::HashMap;

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
    Race {
        #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
        timeout_ms: u64,
    },
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

    pub(crate) fn from_db_string(s: &str) -> Self {
        if s == "follow_global" {
            return StrategyConfig::FollowGlobal;
        }
        serde_json::from_str(s).unwrap_or(StrategyConfig::FollowGlobal)
    }

    fn validate(&self) -> Result<(), &'static str> {
        match self {
            StrategyConfig::Race { timeout_ms } if *timeout_ms == 0 || *timeout_ms > 300_000 => {
                Err("race timeout_ms must be between 1 and 300000")
            }
            StrategyConfig::LoadBalance { weight_mode }
                if weight_mode != "equal" && weight_mode != "priority" =>
            {
                Err("load_balance weight_mode must be 'equal' or 'priority'")
            }
            StrategyConfig::Shadow { shadow_target } if shadow_target.trim().is_empty() => {
                Err("shadow_target must not be empty")
            }
            _ => Ok(()),
        }
    }
}

fn parse_global_strategy(value: &str) -> Result<StrategyConfig, String> {
    let strategy = match value {
        "failover" => StrategyConfig::Failover,
        "race" => StrategyConfig::Race { timeout_ms: 5_000 },
        "load_balance" => StrategyConfig::LoadBalance { weight_mode: "priority".into() },
        raw if raw.starts_with('{') => serde_json::from_str::<StrategyConfig>(raw)
            .map_err(|error| format!("invalid strategy JSON: {error}"))?,
        _ => return Err("default_strategy must be failover, race, load_balance, or a StrategyConfig JSON object".into()),
    };
    if matches!(&strategy, StrategyConfig::FollowGlobal) {
        return Err("default strategy cannot follow itself".into());
    }
    strategy.validate().map_err(str::to_string)?;
    Ok(strategy)
}

pub(crate) fn routing_strategy(
    state: &AppState,
    endpoint_id: &str,
) -> jev_core::adapter::RoutingStrategy {
    let endpoint_strategy = state.service_endpoints.read().ok().and_then(|endpoints| {
        endpoints
            .get(endpoint_id)
            .map(|endpoint| StrategyConfig::from_db_string(&endpoint.strategy_config))
    });
    let config = match endpoint_strategy {
        Some(StrategyConfig::FollowGlobal) | None => {
            let global = state
                .db_conn
                .lock()
                .ok()
                .and_then(|conn| crate::db::endpoints::get_default_strategy(&conn).ok())
                .unwrap_or_else(|| "failover".into());
            match global.as_str() {
                "race" => StrategyConfig::Race { timeout_ms: 5_000 },
                "load_balance" => StrategyConfig::LoadBalance {
                    weight_mode: "priority".into(),
                },
                _ if global.starts_with('{') => StrategyConfig::from_db_string(&global),
                _ => StrategyConfig::Failover,
            }
        }
        Some(config) => config,
    };
    match config {
        StrategyConfig::FollowGlobal | StrategyConfig::Failover => {
            jev_core::adapter::RoutingStrategy::Failover
        }
        StrategyConfig::Race { timeout_ms } => {
            jev_core::adapter::RoutingStrategy::Race { timeout_ms }
        }
        StrategyConfig::LoadBalance { weight_mode } => {
            jev_core::adapter::RoutingStrategy::LoadBalance { weight_mode }
        }
        StrategyConfig::Shadow { shadow_target } => {
            jev_core::adapter::RoutingStrategy::Shadow { shadow_target }
        }
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
    pub unknown: u32,
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
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub calls_24h: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub calls_total: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub created_at: i64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
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
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
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
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    #[serde(default)]
    pub id: Option<String>, // 修改 ID（可选）
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    #[serde(default)]
    pub strategy_config: Option<StrategyConfig>,
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    #[cfg_attr(feature = "ts-rs", ts(optional = nullable))]
    pub routes: Option<Vec<jev_core::router::RouteEdge>>,
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

/// Until route health probes exist, report every configured route as unknown.
fn unknown_health_summary(routes_count: u32) -> HealthSummary {
    HealthSummary {
        total: routes_count,
        healthy: 0,
        degraded: 0,
        failed: 0,
        unknown: routes_count,
    }
}

/// Real request counts from the call log. The caller owns the DB guard to avoid re-locking.
fn compute_calls(conn: &rusqlite::Connection, endpoint_id: &str) -> (u64, u64) {
    let total = conn
        .query_row(
            "SELECT COUNT(*) FROM call_logs WHERE endpoint_id = ?",
            [endpoint_id],
            |row| row.get::<_, u64>(0),
        )
        .unwrap_or(0);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        - 24 * 60 * 60;
    let recent = conn
        .query_row(
            "SELECT COUNT(*) FROM call_logs WHERE endpoint_id = ? AND timestamp >= ?",
            rusqlite::params![endpoint_id, since],
            |row| row.get::<_, u64>(0),
        )
        .unwrap_or(0);
    (recent, total)
}

/// 统计入口的路由数量
fn count_routes(state: &AppState, endpoint_id: &str) -> u32 {
    state
        .db_conn
        .lock()
        .ok()
        .and_then(|conn| crate::db::load_runtime_snapshot(&conn).ok().flatten())
        .map(|snapshot| {
            snapshot
                .routes
                .iter()
                .filter(|edge| edge.left == endpoint_id)
                .count() as u32
        })
        .unwrap_or_else(|| {
            state
                .registry
                .router()
                .edges()
                .iter()
                .filter(|e| e.left == endpoint_id)
                .count() as u32
        })
}

/// Merge endpoint-owned SQLite routes into the already loaded base graph. Disabled
/// endpoints remain persisted but cannot contribute executable edges.
pub(crate) fn refresh_endpoint_routes(
    state: &AppState,
    retired_ids: &[String],
) -> Result<(), String> {
    let conn = state.db_conn.lock().map_err(|e| e.to_string())?;
    let endpoints = crate::db::endpoints::load_all(&conn).map_err(|e| e.to_string())?;
    let endpoint_ids: std::collections::HashSet<String> =
        endpoints.iter().map(|e| e.id.clone()).collect();
    let enabled: std::collections::HashSet<String> = endpoints
        .iter()
        .filter(|e| e.enabled)
        .map(|e| e.id.clone())
        .collect();
    let snapshot = crate::db::load_runtime_snapshot(&conn)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "runtime config snapshot is missing".to_string())?;
    drop(conn);

    let edges: Vec<_> = snapshot
        .routes
        .into_iter()
        .filter(|edge| {
            (!endpoint_ids.contains(&edge.left) || enabled.contains(&edge.left))
                && !retired_ids.contains(&edge.left)
        })
        .collect();
    state.registry.replace_edges(edges);
    Ok(())
}

fn validate_endpoint_routes(
    id: &str,
    routes: &[jev_core::router::RouteEdge],
) -> Result<(), String> {
    if routes
        .iter()
        .any(|edge| edge.left != id || edge.left.trim().is_empty() || edge.right.trim().is_empty())
    {
        return Err("each endpoint route must have this endpoint as its left node and a non-empty right node".into());
    }
    Ok(())
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

    let endpoints: Vec<crate::db::endpoints::ServiceEndpoint> =
        match crate::db::endpoints::load_all(&conn) {
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
    let runtime_snapshot = crate::db::load_runtime_snapshot(&conn).ok().flatten();
    let route_counts: HashMap<String, u32> = runtime_snapshot
        .as_ref()
        .map(|snapshot| {
            let mut counts = HashMap::new();
            for edge in &snapshot.routes {
                *counts.entry(edge.left.clone()).or_insert(0) += 1;
            }
            counts
        })
        .unwrap_or_default();
    let call_counts: HashMap<String, (u64, u64)> = endpoints
        .iter()
        .map(|endpoint| (endpoint.id.clone(), compute_calls(&conn, &endpoint.id)))
        .collect();
    // Release the DB guard before `count_routes` performs its own short query per
    // endpoint. Re-locking a std::sync::Mutex on this task would deadlock.
    drop(conn);

    let views: Vec<ServiceEndpointView> = endpoints
        .into_iter()
        .map(|ep| {
            let routes_count = route_counts
                .get(&ep.id)
                .copied()
                .unwrap_or_else(|| count_routes(&state, &ep.id));
            let health_summary = unknown_health_summary(routes_count);
            let (calls_24h, calls_total) = call_counts.get(&ep.id).copied().unwrap_or((0, 0));

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
pub async fn create_endpoint(State(state): State<AppState>, body: Bytes) -> Response {
    let req: CreateEndpointRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid body: {e}"),
            )
        }
    };
    if req.id.trim().is_empty() {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "endpoint id must not be empty",
        );
    }
    if let Err(message) = req.strategy_config.validate() {
        return err(&state, StatusCode::BAD_REQUEST, message);
    }

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
    let mut conn = match state.db_conn.lock() {
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

    // Snapshot routes and endpoint metadata are one SQLite transaction.
    if let Some(routes) = &req.routes {
        if let Err(message) = validate_endpoint_routes(&req.id, routes) {
            let _ = tx.rollback();
            return err(&state, StatusCode::BAD_REQUEST, message);
        }
    }
    let mut snapshot = match crate::db::load_runtime_snapshot(&tx) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            let _ = tx.rollback();
            return err(
                &state,
                StatusCode::CONFLICT,
                "runtime config snapshot is missing",
            );
        }
        Err(e) => {
            let _ = tx.rollback();
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load runtime snapshot failed: {e}"),
            );
        }
    };
    snapshot.routes.retain(|edge| edge.left != req.id);
    snapshot
        .routes
        .extend(req.routes.clone().unwrap_or_default());
    if let Err(message) = super::validate_route_graph(&snapshot.routes, &snapshot.providers) {
        let _ = tx.rollback();
        return err(&state, StatusCode::BAD_REQUEST, message);
    }
    if let Err(e) = crate::db::save_runtime_snapshot(&tx, &snapshot) {
        let _ = tx.rollback();
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save endpoint routes failed: {e}"),
        );
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
    drop(conn);
    if let Err(e) = refresh_endpoint_routes(&state, &[]) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("refresh endpoint routes failed: {e}"),
        );
    }

    let routes_count = req
        .routes
        .as_ref()
        .map(|routes| routes.len() as u32)
        .unwrap_or(0);
    Json(ServiceEndpointView {
        id: req.id.clone(),
        strategy_config: req.strategy_config,
        enabled: true,
        routes_count,
        health_summary: unknown_health_summary(routes_count),
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
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid body: {e}"),
            )
        }
    };
    if let Some(strategy) = req.strategy_config.as_ref() {
        if let Err(message) = strategy.validate() {
            return err(&state, StatusCode::BAD_REQUEST, message);
        }
    }

    let mut conn = match state.db_conn.lock() {
        Ok(c) => c,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };

    // Update endpoint metadata, references and route replacement atomically.
    let new_id_ref = req.id.as_deref();
    let strategy_str = req.strategy_config.as_ref().map(|s| s.to_db_string());
    let strategy_ref = strategy_str.as_deref();
    match crate::db::endpoints::get_by_id(&conn, &id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return err(
                &state,
                StatusCode::NOT_FOUND,
                format!("endpoint not found: {id}"),
            )
        }
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load endpoint failed: {e}"),
            )
        }
    }
    let final_id = new_id_ref
        .filter(|next| next != &id)
        .unwrap_or(&id)
        .to_string();
    if final_id.trim().is_empty() {
        return err(
            &state,
            StatusCode::BAD_REQUEST,
            "endpoint id must not be empty",
        );
    }
    if final_id != id
        && crate::db::endpoints::get_by_id(&conn, &final_id)
            .ok()
            .flatten()
            .is_some()
    {
        return err(
            &state,
            StatusCode::CONFLICT,
            format!("endpoint id already exists: {final_id}"),
        );
    }
    if let Some(routes) = &req.routes {
        if let Err(message) = validate_endpoint_routes(&final_id, routes) {
            return err(&state, StatusCode::BAD_REQUEST, message);
        }
    }
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("transaction failed: {e}"),
            )
        }
    };
    if let Err(e) =
        crate::db::endpoints::update(&tx, &id, Some(&final_id), strategy_ref, req.enabled)
    {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("update endpoint failed: {e}"),
        );
    }
    let mut snapshot = match crate::db::load_runtime_snapshot(&tx) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            let _ = tx.rollback();
            return err(
                &state,
                StatusCode::CONFLICT,
                "runtime config snapshot is missing",
            );
        }
        Err(e) => {
            let _ = tx.rollback();
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load runtime snapshot failed: {e}"),
            );
        }
    };
    if final_id != id {
        for edge in &mut snapshot.routes {
            if edge.left == id {
                edge.left = final_id.clone();
            }
            if edge.right == id {
                edge.right = final_id.clone();
            }
        }
    }
    if let Some(routes) = &req.routes {
        snapshot.routes.retain(|edge| edge.left != final_id);
        snapshot.routes.extend(routes.clone());
    }
    if let Err(message) = super::validate_route_graph(&snapshot.routes, &snapshot.providers) {
        let _ = tx.rollback();
        return err(&state, StatusCode::BAD_REQUEST, message);
    }
    if let Err(e) = crate::db::save_runtime_snapshot(&tx, &snapshot) {
        let _ = tx.rollback();
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("save endpoint routes failed: {e}"),
        );
    }
    if let Err(e) = tx.commit() {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("commit endpoint update failed: {e}"),
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
    drop(conn);
    let retired = if final_id != id {
        vec![id.clone()]
    } else {
        Vec::new()
    };
    if let Err(e) = refresh_endpoint_routes(&state, &retired) {
        return err(
            &state,
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("refresh endpoint routes failed: {e}"),
        );
    }

    Json(serde_json::json!({"updated": true})).into_response()
}

/// DELETE /v1/admin/endpoints/{id} — 删除入口 + 级联删除路由
pub async fn delete_endpoint(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let mut conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db lock failed: {e}"),
            )
        }
    };
    match crate::db::endpoints::get_by_id(&conn, &id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return err(
                &state,
                StatusCode::NOT_FOUND,
                format!("endpoint not found: {id}"),
            )
        }
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("load endpoint failed: {e}"),
            )
        }
    }
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => {
            return err(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("transaction failed: {e}"),
            )
        }
    };
    match crate::db::endpoints::delete(&tx, &id) {
        Ok(_) => {
            let mut snapshot = match crate::db::load_runtime_snapshot(&tx) {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => {
                    let _ = tx.rollback();
                    return err(
                        &state,
                        StatusCode::CONFLICT,
                        "runtime config snapshot is missing",
                    );
                }
                Err(e) => {
                    let _ = tx.rollback();
                    return err(
                        &state,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("load runtime snapshot failed: {e}"),
                    );
                }
            };
            let original_count = snapshot.routes.len();
            let mut removed_nodes = std::collections::HashSet::from([id.clone()]);
            // Cascade incoming references only along branches disconnected by this
            // deletion. Unrelated provider paths and intermediate nodes stay intact.
            loop {
                let mut affected = std::collections::HashSet::new();
                snapshot.routes.retain(|edge| {
                    let keep =
                        !removed_nodes.contains(&edge.left) && !removed_nodes.contains(&edge.right);
                    if !keep {
                        affected.insert(edge.left.clone());
                    }
                    keep
                });
                let remaining_lefts: std::collections::HashSet<_> = snapshot
                    .routes
                    .iter()
                    .filter(|edge| edge.r#match == jev_core::router::MatchMode::Exact)
                    .map(|edge| edge.left.as_str())
                    .collect();
                removed_nodes = affected
                    .into_iter()
                    .filter(|node| {
                        !snapshot.providers.contains_key(node)
                            && !remaining_lefts.contains(node.as_str())
                            && snapshot.routes.iter().any(|edge| edge.right == *node)
                    })
                    .collect();
                if removed_nodes.is_empty() {
                    break;
                }
            }
            let routes_deleted = (original_count - snapshot.routes.len()) as u32;
            if let Err(message) = super::validate_route_graph(&snapshot.routes, &snapshot.providers)
            {
                let _ = tx.rollback();
                return err(&state, StatusCode::BAD_REQUEST, message);
            }
            if let Err(e) = crate::db::save_runtime_snapshot(&tx, &snapshot) {
                let _ = tx.rollback();
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("save endpoint delete snapshot failed: {e}"),
                );
            }
            if let Err(e) = tx.commit() {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("commit endpoint delete failed: {e}"),
                );
            }
            // 更新内存
            {
                let mut endpoints = state.service_endpoints.write().expect("endpoints lock");
                endpoints.remove(&id);
            }
            drop(conn);
            if let Err(e) = refresh_endpoint_routes(&state, &[id.clone()]) {
                return err(
                    &state,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("refresh endpoint routes failed: {e}"),
                );
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
pub async fn update_default_strategy(State(state): State<AppState>, body: Bytes) -> Response {
    let req: DefaultStrategyBody = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return err(
                &state,
                StatusCode::BAD_REQUEST,
                format!("invalid body: {e}"),
            )
        }
    };
    if let Err(message) = parse_global_strategy(&req.default_strategy) {
        return err(&state, StatusCode::BAD_REQUEST, message);
    }

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
