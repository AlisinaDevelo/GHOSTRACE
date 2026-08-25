# Task 0061 evidence: cursor monotonicity and idempotent replay

Status: complete for the verified fixture and journal cursor contract. Live
collection remains explicitly disabled; this receipt does not claim production
collector coverage, throughput, or release signing.

## Scope and implementation

Implementation PR [#205](https://github.com/AlisinaDevelo/GHOSTRACE/pull/205)
was merged to protected `main` on 2026-08-25 at
`8bca31bea7ee0c12bfc972bc1bdc27bc0017e71a`. The implementation commit was
`d9f94129eedf59f237219cd0e6aa9cf1b7933697`; the follow-up task-ledger index
fix was `778b0ec6a4fcba94e93c99b046b47d9b160c8916`.

The public cursor contract in `src/cursor.rs` binds every cursor to an event
source and collector instance. It models opaque, sequence, reset, and wrap
tokens; ordered, duplicate, regressed, skipped, and unknown transitions; and
active, reset, wrapped, and invalidated state. `Journal` commits cursor state
with the event in one transaction, returns the original ingest sequence for an
exact replay, and fails closed for a divergent same-cursor event, regression,
unknown ordering, skipped ranges, invalidated state, or a policy change without
an explicit reset. Migration `0003_cursor_contract.sql` persists the state and
backfills existing rows safely.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Cursor types define source identity, comparison, reset, wrap, and invalidation semantics | `src/cursor.rs`; `cursor_identity_and_ordering_are_typed`; reset/wrap/invalidate integration coverage |
| Duplicate deliveries are idempotent while divergent events at the same cursor fail closed | `Journal::ingest`; `journal_replay_is_idempotent_and_conflicts_fail_closed`; migration and writer suites |
| Property-style coverage exercises reordering, replay, crash, source replacement, and policy-version changes | `tests/cursor_contract.rs`: deterministic ordering/reordering/replay, source replacement, policy mismatch, reopen/crash recovery, reset/wrap controls |

The tests also cover explicit lifecycle/gap discontinuities, cursor identity
replacement, malformed tokens, sequence skips, invalidation, migration
idempotence, and the existing privacy, WAL, export, schema, and capture-refusal
boundaries.

## Source-device pipe

The source run was executed from implementation commit
`d9f94129eedf59f237219cd0e6aa9cf1b7933697` on the target device. The later
task-ledger index change was documentation-only and was validated by the
protected-main pipe below.

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |
| Full source pipe | `/private/tmp/ghostrace-0061-source-pipe.1787620250.log` (552 lines) |
| Source pipe SHA-256 | `8a37448ce655b3cf8cfbc7c30b519d178eab4c3b6d27e8584d3bed99de576c56` |
| Release-focused receipt | `/private/tmp/ghostrace-0061-release-focused.1787620500.log` (105 lines) |
| Release-focused SHA-256 | `b45f158ce0b6a520a0c61e1aac1dfa84b4f8f2fcc95b91aaaf064281da3bf3bd` |
| Release all-target receipt | `/private/tmp/ghostrace-0061-release-all-targets.1787620700.log` (197 lines) |
| Release all-target SHA-256 | `a785b9bdec786e09bdf9cb01010278ae5de0c7f5999bc482f9e6801f94ab170b` |

The source pipe used locked offline dependencies, one build job, no incremental
artifacts, format checking, all-target/all-feature checking and Clippy with
`-D warnings`, debug tests, doctests, fixture/identity/roadmap/release-evidence
checks, 38 Python tests, reproducibility, ShellCheck, actionlint, network-denial
and sandbox lanes. It ended `SOURCE_PIPE_PASS`. The focused release receipt
ran the cursor, migration, and writer integration suites; the complete release
all-target/all-feature suite also ended `RELEASE_ALL_TARGETS_PASS`.

## Protected-main reproduction

The complete reproduction ran from clean protected main at
`8bca31bea7ee0c12bfc972bc1bdc27bc0017e71a` with the same device, OS,
architecture, toolchain, locked inputs, offline setting, and one-job limit.

| Field | Value |
| --- | --- |
| Full merged-main pipe | `/private/tmp/ghostrace-0061-merged-pipe.1787620800.log` (553 lines) |
| Merged-main pipe SHA-256 | `457cf84627e41ab2d32caf7b7df5f2039e107f09ab9a1ac0d5e8de323de055bb` |
| Result | `MERGED_PIPE_PASS` |

The merged-main pipe repeated the full debug all-target/all-feature and static
suite: unit, cursor, migrations, privacy, support, vertical-slice, WAL, and
writer tests; doctests; fixture/identity/roadmap/release-evidence checks; 38
Python tests; reproducibility; ShellCheck; actionlint; network-denial; and
sandbox lanes. The debug test inventory was 8 unit, 5 cursor, 7 migration, 1
privacy, 5 support, 26 vertical-slice, 6 WAL, and 5 writer tests, plus the
existing ignored canary. The deterministic demo/export reproduction was run
twice and matched.

Hosted PR checks were green, but acceptance is based on the retained device
receipts above. Intel macOS, the macOS 15 floor, signed/notarized distribution,
live Keychain capture, live collectors, and production throughput remain
unverified or explicit no-go scope.
