CREATE TABLE IF NOT EXISTS authenticated_state (
    state_key TEXT PRIMARY KEY,
    schema_version INTEGER NOT NULL,
    chain_epoch INTEGER NOT NULL,
    chain_start_mac TEXT NOT NULL,
    head_mac TEXT NOT NULL,
    key_generation INTEGER NOT NULL,
    event_count INTEGER NOT NULL,
    max_ingest_seq INTEGER NOT NULL,
    event_order_digest TEXT NOT NULL,
    event_set_digest TEXT NOT NULL,
    event_content_digest TEXT NOT NULL,
    cursor_digest TEXT NOT NULL,
    policy_digest TEXT NOT NULL,
    diagnostic_digest TEXT NOT NULL,
    deletion_count INTEGER NOT NULL,
    deletion_digest TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (state_key = 'journal')
);

INSERT OR IGNORE INTO journal_metadata(metadata_key, metadata_value)
VALUES ('authenticated_state_bootstrap', 'pending');
