//! Database schema initialization and migrations.

use rusqlite::{Connection, Result};

/// 执行数据库迁移（读取 SQL 脚本并执行）
pub fn run_migrations(conn: &Connection) -> Result<()> {
    // 检查 migrations 表是否存在
    let migrations_exist: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='migrations'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)?;

    if !migrations_exist {
        // 首次初始化：执行初始 schema
        let schema_sql = include_str!("../../../migrations/001_initial_schema.sql");
        conn.execute_batch(schema_sql)?;
        tracing::info!("database schema initialized (version 1)");
    } else {
        // 检查是否需要执行新的迁移
        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            let schema_sql = include_str!("../../../migrations/001_initial_schema.sql");
            conn.execute_batch(schema_sql)?;
            tracing::info!("database migrated to version 1");
        }
    }

    Ok(())
}
