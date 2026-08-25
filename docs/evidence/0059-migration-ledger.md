# Task 0059 evidence: checksummed migrations and unsafe-downgrade refusal

Status: complete for the verified fixture migration contract.

Task 0059 makes the SQLite schema boundary deterministic. The fixture journal now
has an ordered embedded catalog, a tamper-evident migration ledger, transactional
upgrade steps, legacy-v1 adoption only after structural verification, and bounded
startup refusals for unsafe state. It does not enable live collection, add a
network client, or make a production capacity claim.

## Retained source artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit | `2fb879e1f310de05dd1ab2c995d7f4a2391c6314` |
| Implementation PR | [#201](https://github.com/AlisinaDevelo/GHOSTRACE/pull/201) |
| Source-device full pipe | `/private/tmp/ghostrace-0059-source-pipe.brtwnc`; SHA-256 `f58e1b20151558337c79d398b6c4e7216878eeb77e33748714396582f19fecae` |
| Focused migration log | `/private/tmp/ghostrace-0059-migration-measurements.rSO877`; SHA-256 `a7550cdc871062f17b3e4a31d84b6ac4a271f04ef85729ed3497f1ba2e6b0440` |
| MVP artifacts | `/private/tmp/ghostrace-0059-mvp.f0FtZD` |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |
| Locked inputs | `Cargo.lock` SHA-256 `0c5de10ae5006ba3c1fe18f156831e2850af7065081ec2b80a3059f1729aa685` |

The final source pipe ran fail-fast with `CARGO_NET_OFFLINE=true`,
`CARGO_INCREMENTAL=0`, one Cargo build job, and ended with `SOURCE_PIPE_PASS`.
An earlier attempt stopped at WAL tests because the device filesystem was full;
that failed log is retained separately and is not used as acceptance evidence. The
target cache was removed as generated output, and the complete pipe was rerun from
the start after space was recovered.

## Migration catalog

| Version | Stable ID | SQL SHA-256 | Schema version |
| ---: | --- | --- | ---: |
| 0 | `0000_migration_ledger` | `7458710fd99043cfe97383b8fe9c3f5cc15c4c9cf2a78bd77c22fec0ae2eb4ff` | 0 |
| 1 | `0001_init` | `60f20d71011bcb55b7e068997026032ca2650d559bc32873f7f7f44ef4c71871` | 1 |
| 2 | `0002_journal_metadata` | `30e136587d0cb3600e7f5ae4511207d47a83c29221d2e4d512fa4c5b44828e02` | 2 |

Every applied row also records `ghostrace/0.0.1` and an RFC3339 application time.
The focused ledger test printed all three records and verified 64-character
hexadecimal checksums, contiguous ordering, and schema version 2.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Applied migrations record stable identifiers, checksums, schema versions, and tool versions | `migrations/0000_migration_ledger.sql`; `src/journal.rs` migration catalog and `AppliedMigration`; `migration_ledger_records_stable_identity_checksum_and_tool_version`; focused log `a7550cdc…` |
| Modified, missing, reordered, partially applied, or future migrations refuse normal startup | `validate_applied_prefix` and final schema gate in `src/journal.rs`; `modified_migration_refuses_startup_without_echoing_path`; `missing_reordered_partial_and_future_migrations_refuse_startup`; focused log `a7550cdc…` |
| Upgrade, crash-at-each-step, backup restore, and unsupported downgrade fixtures run in CI | `legacy_v1_database_upgrades_and_reopens_idempotently`; `crash_after_each_migration_step_recovers_transactionally`; `backup_restore_preserves_migration_ledger`; `unsupported_downgrade_refuses_startup`; source and release suites |

Negative and recovery coverage includes checksum tampering, record deletion,
record reordering, a future version, a schema row without its ledger record,
`PRAGMA user_version` downgrade, malformed legacy shape, crash after each SQL step,
transaction rollback, legacy upgrade, database-only restore, and error non-echo of
the journal path. Crash children abort before the migration transaction commits;
the parent then reopens and completes the catalog.

## MVP regression and device pipe

The same source commit passed formatting, locked metadata, all-target/all-feature
debug and release suites, doctests, Clippy, documentation, roadmap/index parity,
38 Python evidence tests, reproducibility/fixture/identity checks, deterministic
schema/demo/export, explicit capture refusal, ShellCheck, actionlint, and the
macOS sandbox network-denial lane.

MVP digests from the source device run:

- schema JSON: `722f8585124dc382fb9546b2bb66daeed7de7549ec4ff80297d19720b458d86d`;
- deterministic 8-event demo explanation: `8e4e78c49f923a2ad6631e012cd7be17f6fc08b65887c171f2a88d4062deeddb`;
- deterministic 8-event JSONL export: `fd47b9b1ba689934748605f1ed50f950a2b3f7da0fca6687e1ade1f4e5a201d5`;
- bounded capture-refusal stderr: `9c30a3395e36f5245b81ad296212fea250f05934ebd8163d4c9bbdb5abef48da`.

Focused migration resource sample: 0.49 s real, 59,047,936-byte maximum
resident set, and 42,419,456-byte peak memory footprint for the seven-test
fixture. This is a local smoke measurement, not a production capacity claim.

`cargo-audit` and `cargo-deny` remain unavailable on this device and are not
substituted with hosted results. Intel macOS, the macOS 15 floor, signed and
notarized distribution, and live collectors remain explicit no-go or unverified
scope. The task remained `review` until the protected-main rerun below; that gate
is now recorded.

## Protected-main rerun

The exact source reproduction was rerun from protected `main` after PR #201
merged. The clean detached worktree used the same MacBook, OS, architecture,
toolchain, locked inputs, offline setting, and one-job limit.

| Artifact | Value |
| --- | --- |
| Protected-main merge | `88dd03564deb995c037666bb17d90dbd877a2151` |
| Merged-main full pipe | `/private/tmp/ghostrace-0059-merged-pipe.rnKa8N`; SHA-256 `ad7f74cff52f02ab690603c0dbaa7b9d60f1233336d3d48c0830eb7339eb7a96` |
| Merged-main focused migration log | `/private/tmp/ghostrace-0059-migration-measurements-merged.x02x2W`; SHA-256 `a72861aa8a9dac37b2c0ff072ee4d1981d316ce11aa3274a95dff4d6f2f7c338` |
| Merged-main MVP artifacts | `/private/tmp/ghostrace-0059-mvp-merged.uxQLNX` |
| Merged-main device | MacBook Pro 17,1; Apple M1; 8 GB; arm64; macOS 26.6.2 (25G83) |
| Merged-main toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |

The merged full pipe ended with `MERGED_PIPE_PASS` and repeated the source
format/check/Clippy, debug/release, docs, roadmap/Python, reproducibility,
fixture/identity, deterministic MVP, ShellCheck, actionlint, and sandboxed
network-denial checks. The merged focused suite passed all seven migration tests;
its resource sample was 0.52 s real, 59,719,680-byte maximum resident set, and
43,140,480-byte peak memory footprint. Merged MVP digests matched the source run:
schema `722f8585…`, demo `8e4e78c4…`, export `fd47b9b1…`, and capture-refusal
stderr `9c30a339…`.

Decision: complete for the fixture migration ledger, upgrade, refusal, crash
recovery, and backup-restore contract on the verified device. Unavailable audit
tools, Intel/macOS 15, signing/notarization, and live collectors remain explicit
no-go or unverified scope and are not represented as CI passes.
