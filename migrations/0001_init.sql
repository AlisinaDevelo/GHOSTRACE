CREATE TABLE IF NOT EXISTS schema_versions (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS events (
    ingest_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL,
    observed_at TEXT NOT NULL,
    ingested_at TEXT NOT NULL,
    source TEXT NOT NULL,
    kind TEXT NOT NULL,
    collector_instance TEXT NOT NULL,
    source_cursor TEXT,
    provenance_version TEXT NOT NULL,
    policy_profile_id TEXT NOT NULL,
    policy_profile_version INTEGER NOT NULL,
    evidence TEXT NOT NULL,
    parent_event_id TEXT REFERENCES events(event_id),
    payload_ciphertext BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS events_observed_at_idx ON events(observed_at);
CREATE INDEX IF NOT EXISTS events_parent_idx ON events(parent_event_id);
CREATE INDEX IF NOT EXISTS events_kind_idx ON events(kind);

CREATE TABLE IF NOT EXISTS cursors (
    source TEXT NOT NULL,
    collector_instance TEXT NOT NULL,
    source_cursor TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (source, collector_instance)
);

CREATE TABLE IF NOT EXISTS policy_metadata (
    profile_id TEXT NOT NULL,
    profile_version INTEGER NOT NULL,
    profile_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY (profile_id, profile_version)
);

CREATE TABLE IF NOT EXISTS diagnostics (
    diagnostic_id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL,
    detail TEXT NOT NULL,
    created_at TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_versions(version, applied_at)
VALUES (1, '1970-01-01T00:00:00Z');
