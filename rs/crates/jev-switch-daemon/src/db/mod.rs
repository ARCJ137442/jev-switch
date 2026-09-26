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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfigSnapshot {
    pub providers: HashMap<String, ProviderConfig>,
    pub routes: Vec<RouteEdge>,
    pub source_toml_fingerprint: String,
}

pub fn load_runtime_snapshot(conn: &Connection) -> Result<Option<RuntimeConfigSnapshot>> {
    let json = conn
        .query_row(
            "SELECT snapshot_json FROM runtime_config WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    json.map(|value| {
        serde_json::from_str(&value).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    })
    .transpose()
}

pub fn save_runtime_snapshot(conn: &Connection, snapshot: &RuntimeConfigSnapshot) -> Result<()> {
    let json = serde_json::to_string(snapshot)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    conn.execute("INSERT INTO runtime_config (id, snapshot_json, updated_at) VALUES (1, ?, strftime('%s','now'))
        ON CONFLICT(id) DO UPDATE SET snapshot_json = excluded.snapshot_json, updated_at = excluded.updated_at", [json])?;
    Ok(())
}

pub fn config_fingerprint(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let canonical = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
        .and_then(|value| {
            let table = value.as_table()?;
            let mut relevant = toml::Table::new();
            for key in ["providers", "routes", "router"] {
                if let Some(value) = table.get(key) {
                    relevant.insert(key.into(), value.clone());
                }
            }
            toml::to_string(&toml::Value::Table(relevant)).ok()
        })
        .unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Load the SQLite snapshot as the provider/route runtime authority, seeding it from
/// TOML only once when no snapshot exists. Returns whether TOML has drifted since the
/// snapshot's last explicit import/export/API update.
pub fn restore_or_seed_runtime_config(
    conn: &Connection,
    config: &mut crate::config::Config,
    config_path: &Path,
) -> Result<bool> {
    let current_fingerprint = config_fingerprint(config_path);
    if let Some(snapshot) = load_runtime_snapshot(conn)? {
        migrate_legacy_public_endpoints(conn, &snapshot.routes, &snapshot.providers)?;
        let drifted = snapshot.source_toml_fingerprint != current_fingerprint;
        config.providers = snapshot.providers;
        config.router.clear();
        config.routes = snapshot.routes;
        if drifted {
            tracing::warn!(path = %config_path.display(), "providers.toml changed since SQLite runtime snapshot; explicit import is required");
        }
        return Ok(drifted);
    }
    let mut routes = config.route_edges();
    // One-time compatibility import: if older versions already persisted route rows
    // for service endpoint IDs, preserve those rows over the TOML representation.
    if let (Ok(endpoints), Ok(legacy_routes)) =
        (endpoints::load_all(conn), endpoints::load_routes(conn))
    {
        let endpoint_ids: std::collections::HashSet<_> =
            endpoints.into_iter().map(|endpoint| endpoint.id).collect();
        for endpoint_id in endpoint_ids {
            let owned: Vec<_> = legacy_routes
                .iter()
                .filter(|edge| edge.left == endpoint_id)
                .cloned()
                .collect();
            if !owned.is_empty() {
                routes.retain(|edge| edge.left != endpoint_id);
                routes.extend(owned);
            }
        }
    }
    let snapshot = RuntimeConfigSnapshot {
        providers: config.providers.clone(),
        routes,
        source_toml_fingerprint: current_fingerprint,
    };
    save_runtime_snapshot(conn, &snapshot)?;
    migrate_legacy_public_endpoints(conn, &snapshot.routes, &snapshot.providers)?;
    config.providers = snapshot.providers;
    config.router.clear();
    config.routes = snapshot.routes;
    Ok(false)
}

/// One-time compatibility migration for legacy TOML-only public model IDs. Root
/// route nodes (exact left IDs with outgoing routes and no incoming alias edge)
/// become published endpoint records. Intermediate aliases such as `fallback-main`
/// are never promoted merely because they have outgoing edges.
fn migrate_legacy_public_endpoints(
    conn: &Connection,
    routes: &[RouteEdge],
    providers: &HashMap<String, ProviderConfig>,
) -> Result<()> {
    let applied: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM data_migrations WHERE name = 'legacy_public_endpoints_v1')",
        [],
        |row| row.get(0),
    )?;
    if applied {
        return Ok(());
    }

    // Older releases wrote this data-migration label into migrations.version=3.
    // Its presence proves the one-time import already ran; keep it as a tombstone
    // so a user-deleted public entry cannot be recreated on the next boot.
    let old_import_marker: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM migrations WHERE version = 3 AND description = 'legacy public endpoint import')",
        [], |row| row.get(0),
    )?;

    conn.execute_batch("SAVEPOINT legacy_public_endpoint_import")?;
    let result = (|| -> Result<()> {
        if !old_import_marker && endpoints::load_all(conn)?.is_empty() {
            let incoming: std::collections::HashSet<&str> =
                routes.iter().map(|edge| edge.right.as_str()).collect();
            let mut roots: Vec<String> = routes
                .iter()
                .filter(|edge| edge.r#match == jev_core::router::MatchMode::Exact)
                .map(|edge| edge.left.clone())
                .filter(|left| !incoming.contains(left.as_str()) && !providers.contains_key(left))
                .collect();
            roots.sort();
            roots.dedup();
            for id in roots {
                endpoints::create(conn, &id, "follow_global", true)?;
            }
        }
        conn.execute(
            "INSERT INTO data_migrations (name, applied_at) VALUES ('legacy_public_endpoints_v1', strftime('%s','now'))",
            [],
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => conn.execute_batch("RELEASE legacy_public_endpoint_import")?,
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO legacy_public_endpoint_import; RELEASE legacy_public_endpoint_import",
            );
            return Err(error);
        }
    }
    Ok(())
}

pub fn replace_snapshot_routes(conn: &Connection, routes: Vec<RouteEdge>) -> Result<()> {
    let mut snapshot =
        load_runtime_snapshot(conn)?.ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
    snapshot.routes = routes;
    save_runtime_snapshot(conn, &snapshot)
}

pub fn persist_runtime_config(
    conn: &Connection,
    config: &crate::config::Config,
    config_path: &Path,
) -> Result<()> {
    let source_fingerprint = load_runtime_snapshot(conn)?
        .map(|snapshot| snapshot.source_toml_fingerprint)
        .unwrap_or_else(|| config_fingerprint(config_path));
    save_runtime_snapshot(
        conn,
        &RuntimeConfigSnapshot {
            providers: config.providers.clone(),
            routes: config.route_edges(),
            source_toml_fingerprint: source_fingerprint,
        },
    )
}

/// Accept the current TOML contents as the source baseline only for explicit import/export.
pub fn accept_toml_baseline(
    conn: &Connection,
    config: &crate::config::Config,
    config_path: &Path,
) -> Result<()> {
    save_runtime_snapshot(
        conn,
        &RuntimeConfigSnapshot {
            providers: config.providers.clone(),
            routes: config.route_edges(),
            source_toml_fingerprint: config_fingerprint(config_path),
        },
    )
}

use crate::config::ProviderConfig;
use anyhow::Context;
use jev_core::router::RouteEdge;
use rusqlite::{Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

/// Resolve the runtime DB per active config instance. An explicit data directory wins;
/// otherwise keep the DB beside its TOML config so temporary/test configs stay isolated.
pub fn db_path_for_config(config_path: &Path) -> PathBuf {
    resolve_db_path(
        config_path,
        std::env::var_os("JEV_SWITCH_DATA_DIR").as_deref(),
    )
}

fn resolve_db_path(config_path: &Path, data_dir: Option<&std::ffi::OsStr>) -> PathBuf {
    if let Some(dir) = data_dir.filter(|value| !value.is_empty()) {
        return PathBuf::from(dir).join("jev-switch.db");
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("jev-switch.db")
}

/// 打开数据库连接（如果文件不存在则创建）。运行配置快照包含明文上游密钥，
/// 因此 Unix 上主数据库必须是 0600；受管数据目录还需是 0700，以保护 SQLite
/// journal/WAL 等伴生文件。任意自定义配置目录不会被静默 chmod。
pub fn open_connection(path: &Path) -> anyhow::Result<Connection> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create database directory {}", parent.display()))?;

        #[cfg(unix)]
        if is_managed_data_dir(parent) {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).with_context(
                || {
                    format!(
                        "failed to restrict database directory {} to 0700",
                        parent.display()
                    )
                },
            )?;
        }

        #[cfg(unix)]
        warn_if_database_dir_is_shared(parent);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // Create new files privately (umask can only make this stricter). For an
        // existing file, chmod it directly so even read-only broad-mode DBs can be
        // tightened before SQLite opens them or creates sidecars.
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create database {}", path.display()));
            }
        }
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to restrict database {} to 0600", path.display()))?;
    }

    #[cfg(windows)]
    tracing::warn!(
        path = %path.display(),
        "SQLite runtime snapshot contains provider secrets; Windows relies on the data directory ACL and does not enforce POSIX 0600/0700 modes"
    );

    Connection::open(path).context("failed to open SQLite database")
}

#[cfg(unix)]
fn is_managed_data_dir(parent: &Path) -> bool {
    if let Some(configured) =
        std::env::var_os("JEV_SWITCH_DATA_DIR").filter(|value| !value.is_empty())
    {
        return Path::new(&configured) == parent;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .is_some_and(|home| home.join(".jev-switch") == parent)
}

#[cfg(unix)]
fn warn_if_database_dir_is_shared(parent: &Path) {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(parent) {
        Ok(metadata) => {
            let mode = metadata.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                tracing::warn!(
                    dir = %parent.display(),
                    mode = format_args!("{mode:o}"),
                    "SQLite runtime snapshot contains provider secrets; custom database directory is accessible to group/other users and was not chmodded"
                );
            }
        }
        Err(error) => {
            tracing::warn!(dir = %parent.display(), %error, "could not inspect SQLite database directory permissions")
        }
    }
}

/// 初始化数据库（执行迁移）
pub fn init_database(conn: &Connection) -> Result<()> {
    schema::run_migrations(conn)
}

#[cfg(test)]
mod path_tests {
    use super::*;
    #[cfg(unix)]
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(unix)]
    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    #[cfg(unix)]
    fn test_dir() -> PathBuf {
        let unique = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "jev-switch-db-permissions-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn config_path_uses_sibling_database_without_process_environment_mutation() {
        let path = Path::new("E:/isolated/alpha/providers.toml");
        assert_eq!(
            resolve_db_path(path, None),
            PathBuf::from("E:/isolated/alpha/jev-switch.db")
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_connection_creates_private_database_and_tightens_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.join("jev-switch.db");

        let conn = open_connection(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        conn.execute_batch("CREATE TABLE permission_probe (value INTEGER);")
            .unwrap();
        drop(conn);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        // An existing, broadly-readable database must be tightened before SQLite
        // opens it or creates any journal/WAL sidecars.
        let conn = open_connection(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(conn
            .query_row("SELECT COUNT(*) FROM permission_probe", [], |row| row
                .get::<_, i64>(0))
            .is_ok());
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
