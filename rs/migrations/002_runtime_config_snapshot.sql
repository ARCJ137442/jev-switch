CREATE TABLE IF NOT EXISTS runtime_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    snapshot_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO migrations (version, description, applied_at)
VALUES (2, 'runtime configuration snapshot', strftime('%s', 'now'));
