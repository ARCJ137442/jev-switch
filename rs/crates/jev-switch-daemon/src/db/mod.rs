//! Database module for service endpoints and call logs persistence.
//!
//! 使用 SQLite + rusqlite 实现服务入口配置持久化：
//! - service_endpoints: 服务入口配置（策略 + 启用状态）
//! - routes: 路由配置（已有，扩展关联）
//! - call_logs: API 调用记录（支持统计）
//! - global_config: 全局配置（默认策略等）
//! - migrations: 迁移版本标记

pub mod endpoints;
pub mod schema;

use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};

/// 数据库文件路径（~/.jev-switch/jev-switch.db）
pub fn db_path() -> PathBuf {
    let config_dir = if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join(".jev-switch")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".jev-switch")
    } else {
        PathBuf::from(".jev-switch")
    };

    config_dir.join("jev-switch.db")
}

/// 打开数据库连接（如果文件不存在则创建）
pub fn open_connection(path: &Path) -> Result<Connection> {
    // 确保目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    Connection::open(path)
}

/// 初始化数据库（执行迁移）
pub fn init_database(conn: &Connection) -> Result<()> {
    schema::run_migrations(conn)
}
