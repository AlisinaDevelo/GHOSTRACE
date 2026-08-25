# Task 0062 evidence: storage crash and fault-injection matrix

Status: complete for the current fixture-only journal recovery contract. The
matrix does not enable live collection, Keychain rotation, or production
crash recovery; those remain separately gated and explicitly unverified.

## Scope and implementation

Implementation [PR #207](https://github.com/AlisinaDevelo/GHOSTRACE/pull/207)
was authored at `7e93d1e14705db882c6cbcfc629e0aec171ffdee` and merged to
protected `main` at
`5eafd694178e61874a9c6db78eadfff1ba5c4adb` on 2026-08-25 at 01:50:02Z.

`src/fault.rs` adds an inert-by-default, occurrence-based `FaultPlan`. Its 28
named points bracket storage open/verification, migration SQL and commits, key
access, event/cursor/diagnostic writes, ingest commits, cursor controls, WAL
checkpoints, and database-only backups. `Return` proves transaction rollback;
`Abort` terminates only the explicitly spawned child process, modelling power
loss or process death. `Journal` constructors and `with_fault_plan` make the
capability explicit, so normal operation cannot inherit a test schedule.

The retained schedule fixture is
`tests/fixtures/fault-schedules-v1.json`: 28 minimized named cases and a
32-seed bound. `tests/fault_matrix.rs` checks committed versus rolled-back
events, cursor state, visible gap records, stable fixture key generation 7,
retry/idempotence after reopen, backup/checkpoint recovery, and child-process
abort recovery. Key rotation and Keychain-unavailable behavior are not claimed;
the current fixture has one deterministic key generation only.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Named points cover before/during/after durable transitions | `FaultPoint::ALL` (28 entries), the JSON schedule fixture, `fixture_names_every_durable_fault_boundary_and_is_bounded`, and the journal hooks in `src/journal.rs` |
| Cases assert events, cursors, key generation, gaps, and retry after restart | `return_fault_matrix_rolls_back_or_leaves_a_retryable_commit`, `bounded_seed_matrix_replays_minimized_schedules`, and `abrupt_faults_recover_after_restart`; each recovery checks event count, cursor token/status, gap count, stable generation, and retry result |
| Bounded CI seeds and minimized regression schedules | 32 deterministic seeds plus 28 checked-in schedules; the integration test is part of the standard all-target suite and all hosted PR checks were green |

The existing migration and WAL crash tests also ran in the same pipe. Expected
child panic output from migration crash fixtures is contained by the parent
test; the parent result is `ok`.

## Source-device verification

The source run used implementation commit `7e93d1e14705db882c6cbcfc629e0aec171ffdee`:

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |
| Full debug/static pipe | `/private/tmp/ghostrace-0062-source-pipe-final.log` |
| Full debug/static SHA-256 | `a8ac1f4fe3d62f9bb0791746cef9c963a3d832c4b1fa937e3d05e5d655c9a0c1` |
| Release all-target pipe | `/private/tmp/ghostrace-0062-release-all-targets-final.log` |
| Release all-target SHA-256 | `22bd2b390fa6966ebba0e832b8bb8bebc7680750519434227ccbcc5ceac5b347` |

The debug/static pipe ended `FAULT_MATRIX_SOURCE_PIPE_FINAL_PASS` with exit 0.
It ran locked offline all-target/all-feature check, Clippy (`-D warnings`),
debug tests, doctests, fixture/identity/release-evidence/roadmap checks, 38
Python tests, ShellCheck, actionlint, deterministic reproducibility, sandbox
network denial, and final diff checks. The release all-target/all-feature
receipt ended `FAULT_MATRIX_RELEASE_FINAL_PASS`; its migration child panics are
the expected crash fixture and the parent integration result is green.

A prior release attempt ran out of device disk while compiling and is not used
as evidence. The task-owned target was removed, then the fail-fast release run
above completed successfully.

## Protected-main reproduction

The same complete debug/static reproduction ran from clean protected main at
`5eafd694178e61874a9c6db78eadfff1ba5c4adb`:

| Field | Value |
| --- | --- |
| Merged-main pipe | `/private/tmp/ghostrace-0062-merged-pipe-final.log` |
| Merged-main SHA-256 | `6f4f62bd7283f6738d9586a08beb952fd091bf77ba99e481e52c7d06f7828041` |
| Result | `FAULT_MATRIX_MERGED_PIPE_PASS` |

The merged-main run repeated the source device, OS, architecture, toolchain,
locked/offline inputs, one-job limit, all debug tests, 32-seed/28-case matrix,
38 Python tests, static checks, reproducibility, ShellCheck, actionlint, and
network-denial lanes. This is the acceptance reproduction for the protected
merge; the release receipt above covers the identical source tree's optimized
all-target execution.

Hosted checks are supplementary. Intel macOS, the macOS 15 floor, live
collectors, real Keychain rotation/loss recovery, signed/notarized distribution,
and production throughput remain unverified or explicit no-go scope.
