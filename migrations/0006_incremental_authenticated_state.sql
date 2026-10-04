-- Version 2 keeps the v1 anchor columns and domains intact for migration
-- verification, but makes the live anchor an explicit incremental contract.
-- Row commitments are keyed by a digest of the logical row key so retention
-- does not preserve raw event identifiers in the authentication side tables.
ALTER TABLE authenticated_state ADD COLUMN operation_seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE authenticated_state ADD COLUMN pending_change_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE authenticated_state ADD COLUMN event_order_context BLOB NOT NULL DEFAULT X'';
ALTER TABLE authenticated_state ADD COLUMN event_content_context BLOB NOT NULL DEFAULT X'';

CREATE TABLE IF NOT EXISTS authenticated_commitments (
    table_name TEXT NOT NULL,
    row_key_digest TEXT NOT NULL,
    row_locator TEXT NOT NULL,
    row_digest TEXT NOT NULL,
    PRIMARY KEY (table_name, row_key_digest),
    CHECK (table_name IN ('events', 'cursors', 'policy_metadata', 'diagnostics'))
);

CREATE TABLE IF NOT EXISTS authenticated_operations (
    operation_seq INTEGER PRIMARY KEY,
    key_generation INTEGER NOT NULL,
    chain_epoch INTEGER NOT NULL,
    previous_head_mac TEXT NOT NULL,
    head_mac TEXT NOT NULL,
    delta_digest TEXT NOT NULL,
    operation_mac TEXT NOT NULL,
    committed_at TEXT NOT NULL
);

-- The operation MAC authenticates this ordered, privacy-safe change list.
-- Logical keys and event payloads never enter the durable operation history;
-- only their domain-separated digests, bounded locators, and row digests do.
CREATE TABLE IF NOT EXISTS authenticated_operation_changes (
    operation_seq INTEGER NOT NULL,
    change_index INTEGER NOT NULL,
    table_name TEXT NOT NULL,
    row_key_digest TEXT NOT NULL,
    row_locator TEXT NOT NULL,
    change_kind TEXT NOT NULL,
    row_digest TEXT,
    PRIMARY KEY (operation_seq, change_index),
    CHECK (table_name IN ('events', 'cursors', 'policy_metadata', 'diagnostics')),
    CHECK (change_kind IN ('set', 'tombstone')),
    CHECK (
        (change_kind = 'set' AND row_digest IS NOT NULL)
        OR (change_kind = 'tombstone' AND row_digest IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS authenticated_operation_changes_key_idx
    ON authenticated_operation_changes(operation_seq, table_name, row_key_digest);

CREATE TABLE IF NOT EXISTS authenticated_pending_changes (
    change_id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name TEXT NOT NULL,
    -- For events this is the ingest sequence, never the event identifier.
    -- Other tables use their bounded logical key.
    row_key TEXT NOT NULL,
    operation TEXT NOT NULL,
    CHECK (table_name IN ('events', 'cursors', 'policy_metadata', 'diagnostics')),
    CHECK (operation IN ('insert', 'update', 'delete'))
);

CREATE INDEX IF NOT EXISTS authenticated_pending_changes_order_idx
    ON authenticated_pending_changes(change_id);

CREATE INDEX IF NOT EXISTS authenticated_commitments_event_locator_idx
    ON authenticated_commitments(table_name, row_locator)
    WHERE table_name = 'events';

CREATE TRIGGER IF NOT EXISTS authenticated_events_ai
AFTER INSERT ON events
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('events', CAST(NEW.ingest_seq AS TEXT), 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_events_au
AFTER UPDATE ON events
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('events', CAST(OLD.ingest_seq AS TEXT), 'delete');
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('events', CAST(NEW.ingest_seq AS TEXT), 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_events_ad
AFTER DELETE ON events
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('events', CAST(OLD.ingest_seq AS TEXT), 'delete');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_cursors_ai
AFTER INSERT ON cursors
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('cursors', NEW.source || char(0) || NEW.collector_instance, 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_cursors_au
AFTER UPDATE ON cursors
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('cursors', OLD.source || char(0) || OLD.collector_instance, 'delete');
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('cursors', NEW.source || char(0) || NEW.collector_instance, 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_cursors_ad
AFTER DELETE ON cursors
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('cursors', OLD.source || char(0) || OLD.collector_instance, 'delete');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_policy_metadata_ai
AFTER INSERT ON policy_metadata
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('policy_metadata', NEW.profile_id || char(0) || NEW.profile_version, 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_policy_metadata_au
AFTER UPDATE ON policy_metadata
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('policy_metadata', OLD.profile_id || char(0) || OLD.profile_version, 'delete');
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('policy_metadata', NEW.profile_id || char(0) || NEW.profile_version, 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_policy_metadata_ad
AFTER DELETE ON policy_metadata
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('policy_metadata', OLD.profile_id || char(0) || OLD.profile_version, 'delete');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_diagnostics_ai
AFTER INSERT ON diagnostics
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('diagnostics', CAST(NEW.diagnostic_id AS TEXT), 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_diagnostics_au
AFTER UPDATE ON diagnostics
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('diagnostics', CAST(OLD.diagnostic_id AS TEXT), 'delete');
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('diagnostics', CAST(NEW.diagnostic_id AS TEXT), 'insert');
END;

CREATE TRIGGER IF NOT EXISTS authenticated_diagnostics_ad
AFTER DELETE ON diagnostics
BEGIN
    INSERT INTO authenticated_pending_changes(table_name, row_key, operation)
    VALUES ('diagnostics', CAST(OLD.diagnostic_id AS TEXT), 'delete');
END;

INSERT OR IGNORE INTO journal_metadata(metadata_key, metadata_value)
VALUES ('authenticated_state_format', 'v2-incremental');
