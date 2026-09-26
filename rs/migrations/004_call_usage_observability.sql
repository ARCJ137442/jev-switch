ALTER TABLE call_logs ADD COLUMN upstream_calls INTEGER;
ALTER TABLE call_logs ADD COLUMN usage_json TEXT;
CREATE INDEX IF NOT EXISTS idx_call_logs_token_id ON call_logs(token_id, id);
INSERT OR IGNORE INTO migrations (version, description, applied_at)
VALUES (4, 'persist returned call usage and upstream dispatch count', strftime('%s', 'now'));
