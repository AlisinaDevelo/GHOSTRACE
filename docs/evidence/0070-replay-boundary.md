# Task 0070 evidence: persist one replay boundary per source and volume

Status: verified on protected main at implementation merge
`1d5f88ccd3c37b6fda23a7a6d69ed788b64e7af4` (PR #252). The evidence/ledger
publication is the follow-up PR for this receipt.

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

The implementation merge was verified on macOS 26.6.2, Apple M1 arm64, with
Rust/Cargo 1.88.0 and Python 3.9.6. Every command below exited zero; the
receipt digest is SHA-256 of the complete command output.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo test --all-targets --all-features` | 31 unit tests plus all integration targets passed; one device Keychain test intentionally ignored because it requires explicit authorization | `ef6c721be5398aad5a389404abd67d48608c5c45c3a3ac621cb220b8df2865e0` |
| `cargo clippy --all-targets --all-features -- -D warnings` | passed | `bfc33773865ecee468d754a05bbdad2fc3de8be2a9eb4b5c9d80e230840b4515` |
| `cargo test --release --all-targets --all-features` | release suite passed; same explicitly authorized Keychain test ignored | `aa01207f17c7b2a49bd2fc152ebdf9b332e89bc105c453278a96a029f3e926a5` |
| `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features` | passed | `4e7dd3062936a64b43c4240274eb3c944ab912c5c7cc1a8c294e11d5f450d705` |
| offline network lane | sandbox-denied canary, privacy fixture, and product suite passed | `2181b032fa011a89a813b61e0ddb6d459cd2ca61dc1fd8e6886cc3907593ebe5` |
| reproducibility lane | pinned-input, fixture, identity, deterministic export, roadmap, Python, and Rust checks passed | `230e3b445539733a69c2922869cc9cc5086c5db821a356d4400e14ab0887f5b3` |
| Python contract suite | 40 tests passed | `5a00341b15220286f1ff06733f48609fb571fbb2f3ad733448c44dc573bf284b` |
| roadmap check | 160 tasks; 43 done, 117 backlog, 0 blocked; 488 dependency edges; 108 parent edges | `7302844574f5eb0ed75744862f906ee9746e6d311c8913ebbd5f18416154f12e` |

The hosted PR #252 checks also passed: macOS and Linux stable, Linux MSRV,
clippy, rustfmt, dependency review, audit, deny, offline fixture, and roadmap.
No path, display name, account data, credential, or capture key is retained.
