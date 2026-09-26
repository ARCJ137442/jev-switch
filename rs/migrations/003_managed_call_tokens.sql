CREATE TABLE IF NOT EXISTS call_tokens (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'readonly')),
    secret_hash TEXT NOT NULL UNIQUE,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER
);

ALTER TABLE call_logs ADD COLUMN token_id TEXT REFERENCES call_tokens(id) ON DELETE SET NULL;
ALTER TABLE call_logs ADD COLUMN cost_usd REAL;
CREATE INDEX IF NOT EXISTS idx_call_logs_token ON call_logs(token_id, timestamp);
INSERT OR IGNORE INTO migrations (version, description, applied_at)
VALUES (3, 'managed call tokens and token attribution', strftime('%s', 'now'));
