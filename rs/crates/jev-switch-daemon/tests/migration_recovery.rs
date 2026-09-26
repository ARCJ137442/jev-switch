use jev_switch_daemon::{config::Config, db};
use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;

fn temp_config(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jev-migration-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("providers.toml");
    std::fs::write(&path, body).unwrap();
    path
}

const ROOT_CONFIG: &str = r#"
[providers.fake]
kind = "laya"
base = "http://127.0.0.1:1"
api_key = "test-only"

[[routes]]
left = "legacy-root"
right = "fake"
priority = 0
"#;

#[test]
fn fresh_schema_imports_legacy_root_and_does_not_republish_deleted_entry() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_database(&conn).unwrap();
    let path = temp_config("fresh", ROOT_CONFIG);
    let mut config = Config::load(&path).unwrap();

    db::restore_or_seed_runtime_config(&conn, &mut config, &path).unwrap();
    assert!(db::endpoints::get_by_id(&conn, "legacy-root")
        .unwrap()
        .is_some());

    // Simulate a deliberate user deletion. Re-reading the same snapshot must not
    // resurrect its TOML-compatible public ID.
    conn.execute("DELETE FROM service_endpoints WHERE id = 'legacy-root'", [])
        .unwrap();
    db::restore_or_seed_runtime_config(&conn, &mut config, &path).unwrap();
    assert!(db::endpoints::get_by_id(&conn, "legacy-root")
        .unwrap()
        .is_none());
}

#[test]
fn legacy_v3_marker_repairs_token_schema_preserves_state_and_prevents_reimport() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../../../migrations/001_initial_schema.sql"))
        .unwrap();
    conn.execute_batch(include_str!(
        "../../../migrations/002_runtime_config_snapshot.sql"
    ))
    .unwrap();

    // This is the colliding marker written by the old data migration. In this
    // database it incorrectly makes the old schema runner skip migration 003.
    conn.execute(
        "INSERT INTO migrations (version, description, applied_at) VALUES (3, 'legacy public endpoint import', 1)",
        [],
    ).unwrap();
    db::endpoints::create(
        &conn,
        "kept-entry",
        "{\"kind\":\"race\",\"timeout_ms\":73}",
        false,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO call_logs (timestamp, endpoint_id, route_key, upstream_provider, upstream_model, success, latency_ms)
         VALUES (10, 'kept-entry', 'kept-entry→fake', 'fake', 'm1', 1, 22)",
        [],
    ).unwrap();
    assert!(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='call_tokens'",
            [],
            |_| Ok(())
        )
        .optional()
        .unwrap()
        .is_none());

    db::init_database(&conn).unwrap();

    assert!(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='call_tokens'",
            [],
            |_| Ok(())
        )
        .optional()
        .unwrap()
        .is_some());
    let columns = |table: &str| -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        stmt.query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let call_columns = columns("call_logs");
    for expected in ["token_id", "cost_usd", "upstream_calls", "usage_json"] {
        assert!(
            call_columns.iter().any(|column| column == expected),
            "missing column {expected}"
        );
    }
    let kept = db::endpoints::get_by_id(&conn, "kept-entry")
        .unwrap()
        .unwrap();
    assert!(!kept.enabled);
    assert_eq!(
        kept.strategy_config,
        "{\"kind\":\"race\",\"timeout_ms\":73}"
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM call_logs", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );

    // The legacy v3 marker means this import already ran before the user removed
    // their public card. Keep that tombstone even though no endpoint row remains.
    conn.execute("DELETE FROM service_endpoints WHERE id = 'kept-entry'", [])
        .unwrap();
    let path = temp_config("legacy-v3", ROOT_CONFIG);
    let mut config = Config::load(&path).unwrap();
    db::restore_or_seed_runtime_config(&conn, &mut config, &path).unwrap();
    assert!(db::endpoints::get_by_id(&conn, "legacy-root")
        .unwrap()
        .is_none());
    assert!(db::endpoints::get_by_id(&conn, "kept-entry")
        .unwrap()
        .is_none());
}
