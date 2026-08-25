# Task 0009 evidence: SQLite WAL schema and migration runner

Status: complete on protected `main` at
`0d60e7b30c7e3c6951ad78a420a88698c6546492`. The production schema, WAL
configuration, path hardening, migration catalog, and focused file-backed
contract test are merged, and the same device matrix was rerun at that exact
SHA.

## Contract and implementation

`src/journal.rs`, `src/storage.rs`, and the checked-in migrations create the
durable events, cursors, policy metadata, diagnostics, migration ledger, and
schema-version boundary. A file-backed journal configures WAL mode, synchronous
`FULL`, foreign keys, bounded sidecar verification, and restrictive directory and
file modes. The migration runner applies the ordered embedded catalog
transactionally and reopens an existing database without replaying or changing
the ledger.

`tests/sqlite_storage.rs` adds two focused executable checks:

1. A file-backed journal reports WAL/FULL/foreign-key settings, all schema and
   migration versions, exact Unix 0700/0600 modes, durable event/cursor state,
   and an empty diagnostics table after an ingest.
2. A close/reopen cycle returns the byte-for-byte equal migration ledger and the
   same schema version/count, proving migration application is idempotent.

The existing migration, WAL, path-hardening, cursor, writer, and vertical-slice
tests retain negative, crash, sidecar, backup, tamper, and replay coverage.

## Acceptance mapping

1. **Durable tables exist.** `migrations/0001_init.sql`, `0002_journal_metadata.sql`,
   and `0003_cursor_contract.sql` define events, cursors, policy metadata,
   diagnostics, schema versions, and migration state. The focused test proves
   the public schema/migration counts and successfully commits an event and
   cursor state.
2. **WAL/FULL/foreign-key settings are enforced.** `Journal::open_fixture` reports
   `wal`, `FULL`, and enabled foreign keys; the focused test asserts each on a
   fresh file-backed database. `tests/wal_policy.rs` covers checkpoints, busy
   readers, bounded WAL growth, sidecars, snapshots, and abrupt recovery.
3. **Restrictive file modes.** The storage boundary creates a 0700 journal
   directory and 0600 database/artifacts, rejects unsafe ownership, links, and
   races, and rechecks sidecars after commits. The focused test asserts the
   exact directory/database modes; `tests/vertical_slice.rs` and storage tests
   cover the remaining artifact and race refusals.
4. **Idempotent migrations.** The focused reopen test compares all four applied
   migration records and schema version/count across close/reopen. The migration
   suite additionally covers upgrades, checksums, missing/reordered/partial and
   future state, unsupported downgrade, backup restore, and crash-at-each-step
   recovery.

## Target-device receipts

All receipts below were run on 2026-08-25 from source commit `2381506` on a
MacBookPro17,1 (Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0, with
Rust/Cargo 1.88.0 and offline locked inputs.

| Lane | Result and retained receipt |
| --- | --- |
| Focused SQLite contract | Pass: 2 tests; `/tmp/ghostrace-0009-source-sqlite-storage.log`; SHA-256 `1e13de2ecbe9cd82a515dcb14c03fcc9b4a938c42570f742a072ca851a96ac09` |
| Debug all-target/all-feature suite | Pass; `/tmp/ghostrace-0009-source-debug.log`; SHA-256 `d9badd4619dd894210873bbd8b646550531008b3cbb7a781ec306566a773487a` |
| Clippy with warnings denied | Pass; `/tmp/ghostrace-0009-source-clippy.log`; SHA-256 `c2d5944b6e3123494b2701770e1b895f1eb3109d144866c1437a7a4367c4939e` |
| Reproducibility/static pipe | Pass: pinned inputs, deterministic schema/demo/export, capture refusal, 40 Python tests, clippy, and all targets; `/tmp/ghostrace-0009-source-repro.log`; SHA-256 `1a8f2307f4decd4c509dfc0147c2e479d779b44486e8f6d9c0c2912c634d5268` |
| Explicit network-denial pipe | Pass: denial canary, privacy regression, and complete product suite under macOS `sandbox-exec`; `/tmp/ghostrace-0009-source-offline.log`; SHA-256 `3f39a4abd61d1122d32e9056b8852bd127f7fdc7c143ab336ac3e15a3479cdc9` |
| Release all-target/all-feature suite | Pass; `/tmp/ghostrace-0009-source-release.log`; SHA-256 `839c0347a0a8d5bfec4d7449ffcdb3041827fe63151ff651cdae563b1eef1fca` |
| Rust documentation | Pass; `/tmp/ghostrace-0009-source-doc.log`; SHA-256 `8ae49c8a3b90cde55226ffe20e818a3931929362c8d22136bc4baf345ba5fdc1` |
| Shell/action lint | Pass; `/tmp/ghostrace-0009-source-shellcheck.log` and `/tmp/ghostrace-0009-source-actionlint.log` (both empty-success receipts) |

The device matrix does not claim live collection, a production capacity target,
Intel/macOS 15 support, signed/notarized distribution, or unavailable audit
tools. Hosted checks are an additional merge gate, not a substitute for the
protected-main device rerun.

## Protected-main rerun receipts

The merge of [PR #228](https://github.com/AlisinaDevelo/GHOSTRACE/pull/228) was
verified at `0d60e7b30c7e3c6951ad78a420a88698c6546492` on the same device before
this task was closed.

| Lane | Result and retained receipt |
| --- | --- |
| Focused SQLite contract | Pass: 2 tests; `/tmp/ghostrace-0009-postmerge-sqlite-storage.log`; SHA-256 `23a4579f1dbc960732fba9d97a5e0d2a0d6844075c0d629c1d0a983437e166c0` |
| Debug all-target/all-feature suite | Pass; `/tmp/ghostrace-0009-postmerge-debug.log`; SHA-256 `9af9599be7605c83a18743b211b25877029a7c9369f072829115d17fccc94e72` |
| Clippy with warnings denied | Pass; `/tmp/ghostrace-0009-postmerge-clippy.log`; SHA-256 `bcb74884a78099501f6bfc31de29d15aae4eb48dcd18d5c99bdde44d75b90089` |
| Explicit network-denial pipe | Pass; `/tmp/ghostrace-0009-postmerge-offline.log`; SHA-256 `c3779f7f0e320d68b902086380ec81bd5e0742d0de624e158fa107cb26f2d231` |
| Release all-target/all-feature suite | Pass; `/tmp/ghostrace-0009-postmerge-release.log`; SHA-256 `9cf0d579aecc61aa1afe71cc4cbbdb69d0ff7de46c3cf574e4adb82a3e160641` |
| Reproducibility/static pipe | Pass: pinned inputs, 40 Python tests, and complete locked suite; `/tmp/ghostrace-0009-postmerge-repro.log`; SHA-256 `07651738daaa8c0c97a890c7f1a852c7643034bc71e8b4526fe4f9bf5efcde0a` |
| Rust documentation | Pass; `/tmp/ghostrace-0009-postmerge-doc.log`; SHA-256 `f8c1da37ac5c70c080ae03a28c70c864277dbd1cfe0f76770f84c8f92089b349` |
| Shell/action lint | Pass; `/tmp/ghostrace-0009-postmerge-shellcheck.log` and `/tmp/ghostrace-0009-postmerge-actionlint.log` (both empty-success receipts) |

## Closure

Issue #13 can be closed against this evidence. The verified scope remains the
fixture file-backed journal: no live collection, capacity target,
Intel/macOS 15, signed/notarized distribution, or unavailable audit tool is
represented as covered.
