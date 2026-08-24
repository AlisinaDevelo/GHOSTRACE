CREATE TABLE IF NOT EXISTS migration_records (
    migration_id TEXT PRIMARY KEY,
    version INTEGER NOT NULL UNIQUE,
    checksum TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    tool_version TEXT NOT NULL,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS migration_state (
    state_key TEXT PRIMARY KEY,
    state_value TEXT NOT NULL
);
