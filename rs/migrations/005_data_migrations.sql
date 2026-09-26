-- Data migrations have named markers separate from schema migration versions.
CREATE TABLE IF NOT EXISTS data_migrations (
    name TEXT PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO migrations (version, description, applied_at)
VALUES (5, 'named data migration markers', strftime('%s', 'now'));
