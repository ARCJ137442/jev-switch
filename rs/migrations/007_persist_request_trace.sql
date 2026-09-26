BEGIN IMMEDIATE;

ALTER TABLE call_logs ADD COLUMN request_id TEXT;
ALTER TABLE call_logs ADD COLUMN http_status INTEGER;
ALTER TABLE call_logs ADD COLUMN route_trace_json TEXT;
CREATE UNIQUE INDEX idx_call_logs_request_id ON call_logs(request_id) WHERE request_id IS NOT NULL;

INSERT INTO migrations (version, description, applied_at)
VALUES (7, 'persist request correlation and sanitized route trace', strftime('%s','now'));

COMMIT;
