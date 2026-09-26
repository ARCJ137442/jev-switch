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
        let schema_sql = include_str!("../../../../migrations/001_initial_schema.sql");
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
            let schema_sql = include_str!("../../../../migrations/001_initial_schema.sql");
            conn.execute_batch(schema_sql)?;
            tracing::info!("database migrated to version 1");
        }
    }

    // Idempotent v2 migration adds the unified runtime config snapshot. Running its
    // CREATE IF NOT EXISTS also repairs databases whose migration marker was partial.
    conn.execute_batch(include_str!(
        "../../../../migrations/002_runtime_config_snapshot.sql"
    ))?;
    let current_version: i32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM migrations",
        [],
        |row| row.get(0),
    )?;
    if current_version < 3 {
        // A pre-release build used version 3 for a data migration. Do not trust
        // the numeric marker alone: recover the call-token schema structurally.
        if !table_exists(conn, "call_tokens")?
            && !column_exists(conn, "call_logs", "token_id")?
            && !column_exists(conn, "call_logs", "cost_usd")?
        {
            conn.execute_batch(include_str!(
                "../../../../migrations/003_managed_call_tokens.sql"
            ))?;
        }
    }
    ensure_call_token_schema(conn)?;
    conn.execute(
        "INSERT OR IGNORE INTO migrations (version, description, applied_at) VALUES (3, 'managed call tokens and token attribution', strftime('%s','now'))",
        [],
    )?;
    let current_version: i32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM migrations",
        [],
        |row| row.get(0),
    )?;
    if current_version < 4 {
        conn.execute_batch(include_str!(
            "../../../../migrations/004_call_usage_observability.sql"
        ))?;
    }
    // Dedicated table for data migrations; their names cannot collide with SQL
    // schema version numbers again.
    conn.execute_batch(include_str!(
        "../../../../migrations/005_data_migrations.sql"
    ))?;
    let current_version: i32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM migrations",
        [],
        |row| row.get(0),
    )?;
    if current_version < 6 {
        // Call history is a durable snapshot. Its endpoint_id must not depend on
        // the lifetime of a mutable service_endpoints row.
        conn.execute_batch(include_str!(
            "../../../../migrations/006_detach_call_history_from_endpoints.sql"
        ))?;
    }
    let current_version: i32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM migrations",
        [],
        |row| row.get(0),
    )?;
    if current_version < 7 {
        // Keep the route metadata and response/log correlation ID beside the
        // durable call row; no request or response body is stored in this trace.
        conn.execute_batch(include_str!(
            "../../../../migrations/007_persist_request_trace.sql"
        ))?;
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_call_token_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS call_tokens (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('admin', 'readonly')),
            secret_hash TEXT NOT NULL UNIQUE,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            last_used_at INTEGER
        );",
    )?;
    if !column_exists(conn, "call_logs", "token_id")? {
        conn.execute_batch("ALTER TABLE call_logs ADD COLUMN token_id TEXT REFERENCES call_tokens(id) ON DELETE SET NULL;")?;
    }
    if !column_exists(conn, "call_logs", "cost_usd")? {
        conn.execute_batch("ALTER TABLE call_logs ADD COLUMN cost_usd REAL;")?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_call_logs_token ON call_logs(token_id, timestamp);",
    )?;
    Ok(())
}
