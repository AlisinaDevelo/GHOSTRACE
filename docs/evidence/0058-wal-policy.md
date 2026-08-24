# Task 0058 evidence: WAL, SHM, checkpoint, and reader policy

Status: complete for the verified fixture file-backed WAL policy contract.

Task 0058 makes the SQLite WAL boundary explicit for the local file-backed
journal. It does not enable live collection, add a permission, add a network
client, or claim a production capacity target. The implementation applies a
bounded connection policy, reports checkpoint frame counts and sidecar bytes,
limits read-only snapshot lifetime, and copies only a checkpointed database for a
backup.

## Retained source artifacts

| Artifact | Value |
| --- | --- |
| Implementation source commit | `e943a0d1c88c7ac9d84f1e9278419de170b3e938` |
| Source-device full pipe log | `/private/tmp/ghostrace-0058-source-pipe.YnITRB`; SHA-256 `e96e420da935bd35a4b0402bbe266d69ca4c83795b6ed112d8c46c0de0a40fd4` |
| Focused measurement log | `/private/tmp/ghostrace-0058-wal-measurements.log`; SHA-256 `657ca9962c5371731560b6d6775acb080b34105745d04b63e0b650bc0e5dbf4a` |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |
| Locked inputs | `Cargo.lock` SHA-256 `0c5de10ae5006ba3c1fe18f156831e2850af7065081ec2b80a3059f1729aa685` |

The full pipe ran with `CARGO_NET_OFFLINE=true`, a single Cargo build job, and
`CARGO_INCREMENTAL=0`. Its final marker is `SOURCE_PIPE_PASS`.

## Policy measurements

The default `WalPolicy` is 1,000 auto-checkpoint pages, a 250 ms busy timeout, a
30,000 ms reader limit, and a 64 MiB WAL limit. The focused test uses a narrowed
policy to exercise refusal paths. The retained measurement log records:

- truncate checkpoint: `busy=false`, zero remaining frames, zero observed WAL
  bytes, and a 65,536-byte limit;
- passive checkpoint during a long reader: 10 frames remained and the checkpoint
  returned `WalCheckpointRefused`;
- the long-reader transaction returned `LongReader { elapsed_ms: 70, max_ms: 25 }`;
- the timing test returned `LongReader { elapsed_ms: 40, max_ms: 10 }`;
- clean shutdown returned a bounded truncate report with zero remaining frames and
  zero WAL bytes.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Checkpoint thresholds, busy handling, long-reader limits, and shutdown behavior are explicit and measured | `src/wal.rs`, `src/journal.rs`; `tests/wal_policy.rs`; focused log `657ca996…`; source full pipe `e96e420d…` |
| WAL and SHM files are verified and never copied independently as a valid backup | `src/storage.rs` sidecar verification and database-only snapshot; `backup_snapshot` rejects sidecar destinations; `database_snapshot_rejects_sidecars_and_reopens_after_truncate_checkpoint` |
| Checkpoint starvation and abrupt termination are bounded or refused | `long_reader_is_refused_and_checkpoint_reports_remaining_frames`; `abrupt_child_exit_recovers_without_uncommitted_schema_or_unbounded_wal`; source and release suites |

Negative and recovery coverage includes invalid policy values, in-memory backup
refusal, sidecar-destination refusal, long-reader rollback, passive checkpoint
starvation, abrupt child termination before commit, database reopen, and clean
truncate shutdown. The snapshot test reopens the copied database and verifies all
8 synthetic fixture events; no WAL or SHM snapshot sidecar is created.

## Existing MVP regression pipe

On the same source commit, the full local pipe also passed formatting, locked
metadata, all-target/all-feature debug and release suites, doctests, Clippy,
Rust documentation, 38 Python roadmap/reproducibility tests, generated-index
parity, deterministic schema/demo/export, explicit capture refusal, ShellCheck,
actionlint, fixture/identity checks, and the macOS `sandbox-exec` network-denial
canary. Synthetic MVP artifact digests remain:

- demo explanation: `8e4e78c49f923a2ad6631e012cd7be17f6fc08b65887c171f2a88d4062deeddb`;
- 8-event JSONL export: `fd47b9b1ba689934748605f1ed50f950a2b3f7da0fca6687e1ade1f4e5a201d5`;
- capture-refusal stderr: `9c30a3395e36f5245b81ad296212fea250f05934ebd8163d4c9bbdb5abef48da`.

`cargo-audit` and `cargo-deny` remain unavailable on this device and are not
substituted with hosted results. Intel macOS, the macOS 15 floor, signed and
notarized distribution, and live collectors remain explicit no-go or unverified
scope. This task must receive the same pipe against the merged `main` SHA before
the task status changes from `review` or the issue closes; the protected-main
rerun below records that gate.

## Protected-main rerun

The exact source reproduction was rerun from the protected `main` merge
`c9fc5bc664e105b7a002c235f6ecdab3a3d05485` in a clean detached worktree on the
same device and toolchain.

| Artifact | Value |
| --- | --- |
| Merged-main full pipe log | `/private/tmp/ghostrace-0058-merged-pipe.XEqRmi`; SHA-256 `42d2b726a93a697a3b0a8b7e14c4be268315f01dd9a18e4606f50f5f030315b6` |
| Merged-main focused measurement log | `/private/tmp/ghostrace-0058-wal-measurements-merged.log`; SHA-256 `6b4acdf8e6131d699dec7a5dc0b3e6aeae2ec8a174461126c5616d194a4a4ba6` |
| Merged-main device | MacBook Pro 17,1; Apple M1; 8 GB; arm64; macOS 26.6.2 (25G83) |
| Merged-main toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin` |

The merged focused log records the same bounded truncate report (zero remaining
frames and zero WAL bytes), 10-frame passive starvation refusal, abrupt-exit
recovery, and reader-limit refusals (`70 ms / 25 ms` and `33 ms / 10 ms`). The
full merged pipe ended with `MERGED_PIPE_PASS` and repeated every source check:
locked metadata, format/check/Clippy, debug and release all-target suites,
doctests, docs, roadmap/index parity, 38 Python tests, reproducibility, fixture
and identity checks, deterministic demo/export/capture refusal, ShellCheck,
actionlint, and the enforced macOS sandbox network-denial lane.

Decision: complete for the fixture file-backed WAL policy contract on the verified
device. Intel/macOS 15, signed/notarized distribution, live collectors, and
`cargo-audit`/`cargo-deny` remain explicit no-go or unavailable scope and are not
represented as covered by this task.
