# Task 0070 evidence: persist one replay boundary per source and volume

Status: implementation complete on the source branch; protected-main commit and
device receipts are added after the implementation and evidence PRs merge.

## Contract

`ReplayBoundary` is a versioned, path-free record containing a live
`CursorIdentity` (source, collector instance, volume identity, and per-host or
per-device stream mode) plus a `ReplayConfiguration`. The configuration stores
the selected-root digest, exclusion digest, `since_when`, latency in
milliseconds, and the file-event setting. Its canonical SHA-256 digest is
available for receipts without exposing paths or display names.

The journal stores the serialized boundary beside the cursor in migration
`0004_replay_boundary.sql`. A cursor read with a different volume or stream
identity returns no resumable state. Ingestion refuses a changed root,
exclusion, latency, since-when, or file-event setting with
`CursorBoundaryMismatch` before writing. Explicit reset/wrap APIs can establish
a new epoch and boundary.

FSEvents events carry their source cursor and are submitted to the writer with
the boundary. When callback event IDs have an admitted numeric gap, the
collector first commits an explicit bounded gap cursor and then the event;
event, cursor, policy, and diagnostics remain one SQLite transaction.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Cursor advancement is atomic with acknowledged event/gap batches | `Journal::ingest_batch_with_boundary`, writer boundary submission, migration `0004`, and `tests/replay_boundary.rs::restart_replays_idempotently_and_advances_atomically_with_boundary`; faulting `CursorBeforeUpdate` leaves zero events and zero cursor rows. |
| Root, latency, since-when, file-event, and exclusion changes invalidate or fork | `tests/replay_boundary.rs::every_replay_setting_change_refuses_without_writing` changes each setting and observes `CursorBoundaryMismatch` with no additional event. |
| Restart has no skipped acknowledgements and duplicate delivery is idempotent | The file-backed test reopens with the same boundary, replays the first event at its original sequence, then commits the next contiguous cursor; the stored boundary round-trips. |

## Limits

This task persists and validates the replay boundary but does not claim full
restart recovery for wrapped, dropped, stale, or invalid FSEvents history.
First-class source-loss reason mapping and reconciliation remain tasks 0071,
0072, and 0015. External detach/snapshot hardware and signed distribution are
not part of this device receipt.

## Target-device verification

Final protected-main SHA, hosted checks, macOS device details, command receipts,
and SHA-256 log digests are retained below after merge. No path, display name,
account data, credential, or capture key is retained.
