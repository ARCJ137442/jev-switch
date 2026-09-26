-- Keep request history after an endpoint is deleted. endpoint_id is a historical
-- label; token attribution remains linked to the managed-token row.
BEGIN IMMEDIATE;

-- AUTOINCREMENT sequence is a durable event cursor high-water mark even when old
-- rows were pruned. Preserve it independently from the highest row still present.
CREATE TEMP TABLE call_logs_v6_old_sequence (seq INTEGER NOT NULL);
INSERT INTO call_logs_v6_old_sequence (seq)
SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'call_logs'), 0);

CREATE TABLE call_logs_v6 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    endpoint_id TEXT NOT NULL,
    route_key TEXT NOT NULL,
    upstream_provider TEXT NOT NULL,
    upstream_model TEXT NOT NULL,
    success INTEGER NOT NULL,
    latency_ms INTEGER NOT NULL,
    error_message TEXT,
    token_id TEXT REFERENCES call_tokens(id) ON DELETE SET NULL,
    cost_usd REAL,
    upstream_calls INTEGER,
    usage_json TEXT
);

INSERT INTO call_logs_v6 (
    id, timestamp, endpoint_id, route_key, upstream_provider, upstream_model,
    success, latency_ms, error_message, token_id, cost_usd, upstream_calls, usage_json
)
SELECT
    id, timestamp, endpoint_id, route_key, upstream_provider, upstream_model,
    success, latency_ms, error_message, token_id, cost_usd, upstream_calls, usage_json
FROM call_logs;

DROP TABLE call_logs;
ALTER TABLE call_logs_v6 RENAME TO call_logs;

UPDATE sqlite_sequence
SET seq = MAX(seq, (SELECT seq FROM call_logs_v6_old_sequence))
WHERE name = 'call_logs';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'call_logs', seq FROM call_logs_v6_old_sequence
WHERE seq > COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'call_logs'), 0);
DROP TABLE call_logs_v6_old_sequence;

CREATE INDEX idx_call_logs_timestamp ON call_logs(timestamp);
CREATE INDEX idx_call_logs_endpoint ON call_logs(endpoint_id, timestamp);
CREATE INDEX idx_call_logs_route ON call_logs(route_key, timestamp);
CREATE INDEX idx_call_logs_success ON call_logs(success, timestamp);
CREATE INDEX idx_call_logs_token ON call_logs(token_id, timestamp);
CREATE INDEX idx_call_logs_token_id ON call_logs(token_id, id);

INSERT INTO migrations (version, description, applied_at)
VALUES (6, 'retain call history after endpoint deletion', strftime('%s', 'now'));

COMMIT;
