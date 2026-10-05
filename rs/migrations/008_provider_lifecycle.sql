-- Provider lifecycle ownership and audit records are deliberately separate from
-- call history and route activity. Command bodies and secret values never enter
-- either table; only an identity hash and stable outcome code are retained.
CREATE TABLE IF NOT EXISTS provider_lifecycle_processes (
    provider_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL,
    pid INTEGER,
    program TEXT NOT NULL,
    command_hash TEXT NOT NULL,
    process_policy TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    state TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_lifecycle_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    execution_id TEXT,
    action TEXT NOT NULL,
    state TEXT NOT NULL,
    outcome TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    finished_at INTEGER NOT NULL,
    pid INTEGER,
    command_hash TEXT,
    message TEXT
);

CREATE INDEX IF NOT EXISTS idx_provider_lifecycle_events_provider
    ON provider_lifecycle_events(provider_id, finished_at);

INSERT OR IGNORE INTO migrations (version, description, applied_at)
VALUES (8, 'provider lifecycle ownership and audit', strftime('%s','now'));
