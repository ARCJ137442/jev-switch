//! Service endpoints CRUD operations.

use rusqlite::{params, Connection, Result, Row};
use serde::{Deserialize, Serialize};

/// 服务入口配置（内存 + 数据库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub id: String,
    pub strategy_config: String, // JSON 或 "follow_global"
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ServiceEndpoint {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(ServiceEndpoint {
            id: row.get(0)?,
            strategy_config: row.get(1)?,
            enabled: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }
}

/// 加载所有服务入口
pub fn load_all(conn: &Connection) -> Result<Vec<ServiceEndpoint>> {
    let mut stmt = conn.prepare(
        "SELECT id, strategy_config, enabled, created_at, updated_at FROM service_endpoints",
    )?;

    let endpoints = stmt
        .query_map([], |row| ServiceEndpoint::from_row(row))?
        .collect::<Result<Vec<_>>>()?;

    Ok(endpoints)
}

/// 根据 ID 查询服务入口
pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<ServiceEndpoint>> {
    let mut stmt = conn.prepare(
        "SELECT id, strategy_config, enabled, created_at, updated_at
         FROM service_endpoints WHERE id = ?",
    )?;

    let result = stmt
        .query_row([id], |row| ServiceEndpoint::from_row(row))
        .optional()?;

    Ok(result)
}

/// 创建服务入口
pub fn create(
    conn: &Connection,
    id: &str,
    strategy_config: &str,
    enabled: bool,
) -> Result<ServiceEndpoint> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO service_endpoints (id, strategy_config, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
        params![id, strategy_config, if enabled { 1 } else { 0 }, now, now],
    )?;

    Ok(ServiceEndpoint {
        id: id.to_string(),
        strategy_config: strategy_config.to_string(),
        enabled,
        created_at: now,
        updated_at: now,
    })
}

/// 更新服务入口
pub fn update(
    conn: &Connection,
    id: &str,
    new_id: Option<&str>,
    strategy_config: Option<&str>,
    enabled: Option<bool>,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // 如果需要修改 ID，先更新关联的 routes
    if let Some(new_id) = new_id {
        if new_id != id {
            conn.execute(
                "UPDATE routes SET left_node = ? WHERE left_node = ?",
                params![new_id, id],
            )?;
        }
    }

    // 构建动态 UPDATE 语句
    let mut updates = vec!["updated_at = ?"];
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

    if let Some(new_id) = new_id {
        updates.push("id = ?");
        params_vec.push(Box::new(new_id.to_string()));
    }

    if let Some(strategy) = strategy_config {
        updates.push("strategy_config = ?");
        params_vec.push(Box::new(strategy.to_string()));
    }

    if let Some(en) = enabled {
        updates.push("enabled = ?");
        params_vec.push(Box::new(if en { 1 } else { 0 }));
    }

    let sql = format!(
        "UPDATE service_endpoints SET {} WHERE id = ?",
        updates.join(", ")
    );
    params_vec.push(Box::new(id.to_string()));

    let params_refs: Vec<&dyn rusqlite::ToSql> =
        params_vec.iter().map(|b| b.as_ref()).collect();

    conn.execute(&sql, params_refs.as_slice())?;

    Ok(())
}

/// 删除服务入口（级联删除关联路由）
pub fn delete(conn: &Connection, id: &str) -> Result<usize> {
    // 删除关联的路由
    conn.execute("DELETE FROM routes WHERE left_node = ?", params![id])?;

    // 删除服务入口（call_logs 通过 FOREIGN KEY ON DELETE CASCADE 自动删除）
    let deleted = conn.execute("DELETE FROM service_endpoints WHERE id = ?", params![id])?;

    Ok(deleted)
}

/// 获取全局默认策略
pub fn get_default_strategy(conn: &Connection) -> Result<String> {
    let strategy: String = conn
        .query_row(
            "SELECT value FROM global_config WHERE key = 'default_strategy'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "failover".to_string());

    Ok(strategy)
}

/// 更新全局默认策略
pub fn set_default_strategy(conn: &Connection, strategy: &str) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO global_config (key, value, updated_at)
         VALUES ('default_strategy', ?, ?)",
        params![strategy, now],
    )?;

    Ok(())
}
