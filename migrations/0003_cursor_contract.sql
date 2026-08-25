ALTER TABLE cursors ADD COLUMN epoch INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cursors ADD COLUMN state TEXT NOT NULL DEFAULT 'active';
ALTER TABLE cursors ADD COLUMN cursor_kind TEXT NOT NULL DEFAULT 'opaque';
ALTER TABLE cursors ADD COLUMN policy_profile_id TEXT;
ALTER TABLE cursors ADD COLUMN policy_profile_version INTEGER;
ALTER TABLE cursors ADD COLUMN last_event_id TEXT REFERENCES events(event_id);

UPDATE cursors
SET policy_profile_id = (
        SELECT policy_profile_id
        FROM events
        WHERE events.source = cursors.source
          AND events.collector_instance = cursors.collector_instance
          AND events.source_cursor = cursors.source_cursor
        ORDER BY ingest_seq DESC
        LIMIT 1
    ),
    policy_profile_version = (
        SELECT policy_profile_version
        FROM events
        WHERE events.source = cursors.source
          AND events.collector_instance = cursors.collector_instance
          AND events.source_cursor = cursors.source_cursor
        ORDER BY ingest_seq DESC
        LIMIT 1
    ),
    last_event_id = (
        SELECT event_id
        FROM events
        WHERE events.source = cursors.source
          AND events.collector_instance = cursors.collector_instance
          AND events.source_cursor = cursors.source_cursor
        ORDER BY ingest_seq DESC
        LIMIT 1
    ),
    cursor_kind = CASE
        WHEN source_cursor GLOB 'seq-[0-9]*-[0-9]*' THEN 'sequence'
        WHEN source_cursor GLOB 'reset-[0-9]*-[0-9]*' THEN 'reset'
        WHEN source_cursor GLOB 'wrap-[0-9]*-[0-9]*' THEN 'wrap'
        ELSE 'opaque'
    END;

CREATE INDEX IF NOT EXISTS events_cursor_idx
    ON events(source, collector_instance, source_cursor);
