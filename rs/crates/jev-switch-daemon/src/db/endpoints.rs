//! Service endpoints CRUD operations.

use jev_core::router::{MatchMode, OnError, RouteEdge, Sticky};
use rusqlite::{params, Connection, OptionalExtension, Result, Row};
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

    // Routes are now stored in the canonical runtime_config snapshot and rewritten
    // by the caller in the same transaction. Preserve historical call logs when
    // renaming the endpoint by creating the new parent first, then moving children.
    let target_id = if let Some(new_id) = new_id.filter(|new_id| *new_id != id) {
        let old = get_by_id(conn, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        conn.execute(
            "INSERT INTO service_endpoints (id, strategy_config, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
            params![
                new_id,
                old.strategy_config,
                if old.enabled { 1 } else { 0 },
                old.created_at,
                now
            ],
        )?;
        conn.execute(
            "UPDATE call_logs SET endpoint_id = ? WHERE endpoint_id = ?",
            params![new_id, id],
        )?;
        conn.execute("DELETE FROM service_endpoints WHERE id = ?", [id])?;
        new_id
    } else {
        id
    };

    // 构建动态 UPDATE 语句
    let mut updates = vec!["updated_at = ?"];
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

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
    params_vec.push(Box::new(target_id.to_string()));

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    conn.execute(&sql, params_refs.as_slice())?;

    Ok(())
}

/// 删除服务入口（级联删除关联路由）
pub fn delete(conn: &Connection, id: &str) -> Result<usize> {
    // Remove both outgoing edges and incoming references so the runtime graph has no dangling ID.
    conn.execute("DELETE FROM routes WHERE left_node = ?", params![id])?;
    conn.execute("DELETE FROM routes WHERE right_node = ?", params![id])?;

    // The v6 history schema intentionally has no endpoint foreign key: old call rows
    // remain queryable with their historical endpoint_id after this row is deleted.
    let deleted = conn.execute("DELETE FROM service_endpoints WHERE id = ?", params![id])?;

    Ok(deleted)
}

/// Load the persisted routing edges. These are merged with the TOML graph at startup.
pub fn load_routes(conn: &Connection) -> Result<Vec<RouteEdge>> {
    let mut stmt = conn.prepare(
        "SELECT left_node, right_node, upstream_model, priority, match_mode, sticky, on_error
         FROM routes ORDER BY left_node, priority, id",
    )?;
    let rows = stmt.query_map([], |row| {
        let match_mode: String = row.get(4)?;
        let sticky: String = row.get(5)?;
        let on_error: String = row.get(6)?;
        Ok(RouteEdge {
            left: row.get(0)?,
            right: row.get(1)?,
            upstream_model: row.get(2)?,
            priority: row.get(3)?,
            r#match: if match_mode == "prefix" {
                MatchMode::Prefix
            } else {
                MatchMode::Exact
            },
            sticky: if sticky == "session" {
                Sticky::Session
            } else {
                Sticky::None
            },
            on_error: if on_error == "fail" {
                OnError::Fail
            } else {
                OnError::Next
            },
        })
    })?;
    rows.collect()
}

/// Replace the outgoing edges owned by one service endpoint within an existing transaction.
pub fn replace_endpoint_routes(
    conn: &Connection,
    endpoint_id: &str,
    routes: &[RouteEdge],
) -> Result<()> {
    conn.execute("DELETE FROM routes WHERE left_node = ?", [endpoint_id])?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    for edge in routes {
        conn.execute(
            "INSERT INTO routes (left_node, right_node, upstream_model, priority, match_mode, sticky, on_error, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![endpoint_id, edge.right, edge.upstream_model, edge.priority,
                if edge.r#match == MatchMode::Prefix { "prefix" } else { "exact" },
                if edge.sticky == Sticky::Session { "session" } else { "none" },
                if edge.on_error == OnError::Fail { "fail" } else { "next" }, now, now],
        )?;
    }
    Ok(())
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

#[cfg(test)]
mod route_persistence_tests {
    use super::*;

    #[test]
    fn endpoint_route_replacement_round_trips_every_edge_field() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_database(&conn).unwrap();
        let first = RouteEdge {
            left: "public".into(),
            r#match: MatchMode::Prefix,
            right: "acct-a".into(),
            upstream_model: Some("upstream/model".into()),
            priority: 7,
            sticky: Sticky::Session,
            on_error: OnError::Fail,
        };
        replace_endpoint_routes(&conn, "public", &[first.clone()]).unwrap();
        assert_eq!(load_routes(&conn).unwrap(), vec![first]);

        let replacement = RouteEdge {
            left: "public".into(),
            r#match: MatchMode::Exact,
            right: "acct-b".into(),
            upstream_model: None,
            priority: 1,
            sticky: Sticky::None,
            on_error: OnError::Next,
        };
        replace_endpoint_routes(&conn, "public", &[replacement.clone()]).unwrap();
        assert_eq!(load_routes(&conn).unwrap(), vec![replacement]);
    }
}
