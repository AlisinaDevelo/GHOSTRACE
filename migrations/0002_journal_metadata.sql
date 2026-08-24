CREATE TABLE IF NOT EXISTS journal_metadata (
    metadata_key TEXT PRIMARY KEY,
    metadata_value TEXT NOT NULL
);

INSERT OR IGNORE INTO journal_metadata(metadata_key, metadata_value)
VALUES ('format', 'ghostrace-journal-v1');
