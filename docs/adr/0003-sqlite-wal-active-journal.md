# ADR 0003: SQLite WAL for the active journal

- **Status:** Accepted
- **Date:** 2026-08-23
- **Scope:** Local persistence

## Context

The journal needs durable local transactions, a single bounded writer, read access
for deterministic explanation, and a straightforward migration path. The source
cursor and the accepted event must advance together or the source must replay them.

## Decision

Use SQLite in write-ahead logging mode for the active journal. Serialize writes
through one bounded writer and allow read-only queries to observe committed state.
Apply schema migrations from an empty database, enable foreign keys, and use durable
transaction settings appropriate to the platform. Commit the event and its source
cursor atomically. Acknowledgement occurs only after commit.

Task 0058 makes the WAL contract explicit for the fixture file-backed journal. The
default connection policy is:

| Bound | Default | Refusal or recovery behavior |
| --- | ---: | --- |
| SQLite auto-checkpoint | 1,000 pages | Checkpoints are observable through their frame counts |
| Busy timeout | 250 ms | SQLite returns a bounded busy error rather than waiting indefinitely |
| Read snapshot lifetime | 30 s | The read transaction rolls back with a `LongReader` refusal |
| WAL sidecar limit | 64 MiB | A checkpoint with remaining frames or an over-limit sidecar refuses |

The policy can be narrowed for a test or deployment, but never widened past the
validated constructor bounds. File-backed readers use a separate SQLite read-only
connection and a deferred transaction. `PASSIVE` checkpoints never pretend that a
long reader was drained; `TRUNCATE` is required before a database-only snapshot.
The snapshot helper copies only the checkpointed database file and rejects `-wal`,
`-shm`, rollback-journal, temporary, and backup sidecar destinations.

SQLite WAL is a storage mechanism, not a security promise. Production payloads must
be authenticated-encrypted before insertion, with a macOS Keychain-backed key and a
deterministic test provider. Database metadata, WAL sidecars, temporary files,
backups, and exports remain in scope for privacy review.

## Alternatives considered

1. **One JSONL append log:** rejected for concurrent readers, transactional cursor
   advancement, migrations, and bounded query semantics.
2. **Embedded server database:** rejected because it adds a process and surface
   area to a local-only tool.
3. **In-memory only:** rejected because restart recovery and durable gaps are part
   of the evidence contract.
4. **SQLite rollback journal only:** rejected as the default because WAL provides a
   clearer active-journal reader/writer model; rollback mode remains a platform
   fallback only if a measured limitation requires it.

## Consequences

Positive:

- Event and cursor commit boundaries are explicit and testable.
- Read-only explanation does not block the writer for ordinary queries.
- Migrations and fixture tests can run on macOS and Linux.

Costs:

- WAL files can retain metadata until checkpointed and must be protected and
  included in deletion/backup procedures. A checkpoint that cannot drain its
  frames is an explicit refusal, not a success with hidden loss.
- One writer requires bounded queues and explicit backpressure.
- SQLite does not make FSEvents complete and does not defend against a same-user
  attacker.
