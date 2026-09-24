// Phase 4: 数据库模块 — SQLite 初始化、迁移、CRUD
//
// 负责：
// 1. 数据库初始化（检测 + 执行 001_initial_schema.sql）
// 2. TOML 配置迁移到数据库（推导服务入口 + 迁移路由）
// 3. 服务入口 CRUD（service_endpoints 表）
// 4. 路由 CRUD（routes 表）
// 5. 调用日志写入（call_logs 表）
// 6. 调用统计查询（24h 滑动窗口）

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_rusqlite::Connection as AsyncConnection;

/// 数据库管理器（异步 wrapper + 内存缓存）
pub struct Database {
    conn: AsyncConnection,
    /// 内存缓存：服务入口配置（id → ServiceEndpoint）
    endpoints_cache: Arc<RwLock<std::collections::HashMap<String, ServiceEndpoint>>>,
}

/// 服务入口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub id: String,
    pub strategy_config: StrategyConfig,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 路由策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StrategyConfig {
    FollowGlobal,
    Failover,
    Race { timeout_ms: Option<u64> },
    LoadBalance { weight_mode: Option<String> },
    Shadow { shadow_target: String },
}

/// 路由配置（从 TOML [[routes]] 迁移）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: i64,
    pub left_node: String,
    pub right_node: String,
    pub upstream_model: Option<String>,
    pub priority: i32,
    pub match_mode: String, // "exact" | "prefix"
    pub sticky: String,     // "none" | "session"
    pub on_error: String,   // "next" | "fail"
    pub created_at: i64,
    pub updated_at: i64,
}

/// API 调用日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallLog {
    pub timestamp: i64,
    pub endpoint_id: String,
    pub route_key: String, // "left→right"
    pub upstream_provider: String,
    pub upstream_model: String,
    pub success: bool,
    pub latency_ms: i64,
    pub error_message: Option<String>,
}

impl Database {
    /// 初始化数据库（检测 + 迁移）
    pub async fn init(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = AsyncConnection::open(db_path.as_ref())
            .await
            .context("Failed to open database")?;

        // 检查是否需要初始化（migrations 表是否存在）
        let needs_init = conn
            .call(|c| {
                let exists: bool = c
                    .query_row(
                        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='migrations'",
                        [],
                        |_| Ok(true),
                    )
                    .optional()?
                    .is_some();
                Ok(!exists)
            })
            .await?;

        if needs_init {
            tracing::info!("Initializing database schema...");
            Self::run_migration_001(&conn).await?;
        }

        let endpoints_cache = Arc::new(RwLock::new(std::collections::HashMap::new()));

        Ok(Self {
            conn,
            endpoints_cache,
        })
    }

    /// 执行初始 schema 迁移（001_initial_schema.sql）
    async fn run_migration_001(conn: &AsyncConnection) -> Result<()> {
        let sql = include_str!("../../../migrations/001_initial_schema.sql");

        conn.call(move |c| {
            c.execute_batch(sql)?;
            Ok(())
        })
        .await
        .context("Failed to execute migration 001")?;

        tracing::info!("Migration 001 applied successfully");
        Ok(())
    }

    /// 记录 API 调用日志
    pub async fn log_call(&self, log: CallLog) -> Result<()> {
        self.conn
            .call(move |c| {
                c.execute(
                    "INSERT INTO call_logs (timestamp, endpoint_id, route_key, upstream_provider, upstream_model, success, latency_ms, error_message)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        log.timestamp,
                        log.endpoint_id,
                        log.route_key,
                        log.upstream_provider,
                        log.upstream_model,
                        log.success as i32,
                        log.latency_ms,
                        log.error_message,
                    ],
                )?;
                Ok(())
            })
            .await
            .context("Failed to insert call log")
    }

    /// 查询调用统计（24h 滑动窗口）
    pub async fn get_call_stats(&self, endpoint_id: String, window_hours: i64) -> Result<CallStats> {
        self.conn
            .call(move |c| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64;
                let window_start = now - window_hours * 3600;

                let total: i64 = c.query_row(
                    "SELECT COUNT(*) FROM call_logs WHERE endpoint_id = ?1 AND timestamp >= ?2",
                    params![endpoint_id, window_start],
                    |r| r.get(0),
                )?;

                let success: i64 = c.query_row(
                    "SELECT COUNT(*) FROM call_logs WHERE endpoint_id = ?1 AND timestamp >= ?2 AND success = 1",
                    params![endpoint_id, window_start],
                    |r| r.get(0),
                )?;

                let slow_count: i64 = c.query_row(
                    "SELECT COUNT(*) FROM call_logs WHERE endpoint_id = ?1 AND timestamp >= ?2 AND latency_ms > 1000",
                    params![endpoint_id, window_start],
                    |r| r.get(0),
                )?;

                Ok(CallStats {
                    total_calls: total,
                    success_calls: success,
                    failed_calls: total - success,
                    slow_calls: slow_count,
                })
            })
            .await
            .context("Failed to query call stats")
    }
}

/// 调用统计聚合结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStats {
    pub total_calls: i64,
    pub success_calls: i64,
    pub failed_calls: i64,
    pub slow_calls: i64, // latency > 1000ms
}
