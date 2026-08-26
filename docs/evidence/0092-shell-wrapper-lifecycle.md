# Task 0092 evidence: shell wrapper lifecycle and exit semantics

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#319](https://github.com/AlisinaDevelo/GHOSTRACE/pull/319)
was squash-merged to protected `main` at
`fe0a8908a893ef47b6b45ab1cb869609a9c099b3`. This task specifies a synthetic
reference harness for a future explicit shell wrapper; it does not ship a shell
executor, PTY, ambient terminal collector, or command capture path.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0092-01 | Tests cover normal exit, signal, exec failure, shell built-in, pipeline, timeout, cancellation, terminal close, and wrapper crash. | [`fixtures/shell-wrapper-lifecycle-v1.json`](../../fixtures/shell-wrapper-lifecycle-v1.json) contains exactly nine deterministic rows. `tests/shell_wrapper_lifecycle.rs` executes every row, including a real missing-executable spawn failure and a child test process that aborts with SIGABRT for the wrapper-crash path. The merged optimized suite reports 7/7 tests passed. |
| E-0092-02 | The wrapper returns the child status according to the documented shell contract. | `ReferenceWrapper` invokes the fixed `/bin/sh -c` contract with cleared environment and null standard streams. `child_status_is_returned_unchanged_for_exit_signal_builtin_and_pipeline` verifies success `0`, non-zero built-in exit `17`, pipeline success `0`, and SIGTERM `15`; `timeout_and_cancellation_are_terminal_signals_with_bounded_cleanup` verifies SIGKILL `9` for both bounded timeout and cancellation. |
| E-0092-03 | Incomplete executions become explicit terminal gaps and never receive a fabricated end time or success status. | Exec failure, terminal close, and wrapper crash are represented by `TerminalEvidence::Gap` with typed reasons. `incomplete_evidence_has_no_end_time_or_fabricated_success` asserts gap JSON has a null `ended_at` and no status, exit code, or signal. The fixture has no completion fields on gap rows. |

## Delivery

- Issue: [#96](https://github.com/AlisinaDevelo/GHOSTRACE/issues/96)
- Implementation PR: [#319](https://github.com/AlisinaDevelo/GHOSTRACE/pull/319)
- Implementation commit before squash: `3e4d10ad7ab11af2c80e1c7aae64245f11886f99`
- Protected-main merge: `fe0a8908a893ef47b6b45ab1cb869609a9c099b3`
- Verification date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2
MacBookPro17,1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
merged source revision: fe0a8908a893ef47b6b45ab1cb869609a9c099b3
```

## Merged-main device verification

Every command in this section ran from the exact protected-main SHA above.
Hosted checks are corroboration; the retained device logs are the acceptance
evidence.

### Deterministic, privacy, failure, and recovery lanes

- `CARGO_BUILD_JOBS=1 scripts/reproducibility-test.sh` exited `0`: the pinned
  19-fixture manifest, schema/golden comparisons, the explicit 7-test lifecycle
  lane, deterministic demo/journal/export/retention/integrity/authenticated-state/
  recovery flows, 46 Python tests, rustfmt, Clippy with `-D warnings`, and all
  non-native Rust targets passed. The script intentionally skips only its
  separately authorized native filesystem benchmark.
- `cargo +1.88.0 build --locked --release` exited `0`.
- `cargo +1.88.0 test --locked --release --test shell_wrapper_lifecycle -- --nocapture`
  exited `0`; all 7 lifecycle tests passed in optimized mode.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `CARGO_BUILD_JOBS=1 cargo +1.88.0 test --locked --release --all-targets
  --all-features -- --test-threads=1` exited `0`; every target passed,
  including the native filesystem benchmark and native-safe FSEvents lifecycle
  row. Existing authorization-gated Keychain and 10-million-record stress rows
  remain intentionally ignored.

The merged local sandboxed debug offline lane also ran the network-denial canary
and privacy regression successfully. It then reached the existing native
filesystem benchmark and failed its unchanged 30-second per-scenario bound after
90.12s (`scenario exceeded bounded run time`, exit `101`). This is an explicit
no-go for the debug resource lane; no limit was weakened and no result was
reported as a pass. The optimized merged-main native run below passed.

### Native device resource receipts

The direct merged-main filesystem command was:

```text
GHOSTRACE_BENCHMARK_REVISION=fe0a8908a893ef47b6b45ab1cb869609a9c099b3 \
  cargo +1.88.0 test --locked --release --test filesystem_benchmark \
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt \
  -- --exact --nocapture
```

It exited `0` after `31.58s`; the two filesystem benchmark tests passed across
all 24 synthetic scenario runs. The path-free receipt recorded maximum latency
`8431.26475ms`, CPU user/system `24174.424/862.283ms`, RSS peak `16498688`,
disk growth `6138488` bytes, and `energy_nj: 0` as observed by the available
telemetry. Event-storm rows surfaced one explicit `cursor_regression` error and
corresponding gaps; no unsupported completeness claim was made.

The direct merged-main FSEvents lifecycle command was:

```text
cargo +1.88.0 test --locked --release --test fsevents_lifecycle_corpus \
  macos::native_safe_storm_lifecycle_runs_publish_loss_order_recovery_and_resource_receipt \
  -- --exact --nocapture
```

It exited `0` after `1.66s`. The path-free receipt recorded 172 observed events,
zero duplicate source events, zero ordering inversions, three successful restart
recoveries, 27 callback batches, zero dropped events, and zero transport
duplicates. The six native-safe scenarios all report path-digest-only evidence;
sleep/wake, logout, and volume-detach remain explicit guarded no-go rows.

## Hosted review and protected merge

PR #319 was pushed from `feature/shell-wrapper-lifecycle` and merged only after
the required CI, formatting, Clippy, policy/deny, advisory, dependency-review,
roadmap, offline-fixture, and Linux/macOS test checks were green. The first
macOS PR run exposed the existing FSEvents `CursorRegression { event_source:
Filesystem }` race; rerunning only that failed job passed, and no implementation
change or bound relaxation was made to hide it.

## Retained artifacts

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `/tmp/ghostrace-0092-device-info.txt` | exact device/toolchain capture | `5214b0e416cfd329fb583c258f8047ecfdb087863f016888db38c6574e883c44` | 329 |
| `/tmp/ghostrace-0092-merged-repro.log` | merged-main deterministic pipe, exit 0 | `52acf68a0c3583aa81b9fbe84a39d6fff26ce88e56dfaa720e0ed529da01edbe` | 39881 |
| `/tmp/ghostrace-0092-merged-release-build.log` | optimized release build, exit 0 | `8fac0df13a8c76e8972f02511aa84a2c09fe98d4f9a7e4afbbfa2899bed9410a` | 3430 |
| `/tmp/ghostrace-0092-merged-lifecycle-release.log` | optimized lifecycle tests, 7/7, exit 0 | `a49c491030e686a23a92b9fa4e1e5cf5f48a54523c50e64ef026fbe738ce08b3` | 2654 |
| `/tmp/ghostrace-0092-merged-rustdoc.log` | rustdoc with warnings denied, exit 0 | `3bf729f14d1ba949dab4da4fd9f5072528a9ad43f4c967af52adea26b85a056c` | 630 |
| `/tmp/ghostrace-0092-merged-release-tests.log` | optimized all-target/all-feature matrix, exit 0 | `bc3ba2cf9cd706815049aa8d41ee03186d19b0f925bb69fc561e59f22a7fe5a7` | 29681 |
| `/tmp/ghostrace-0092-merged-native-benchmark.log` | optimized native benchmark receipt, exit 0 | `dd5a72979cbf1ad5f4f54907d142f05dcb1d76bd63dbfd6cb239a603fa59b0d4` | 5745 |
| `/tmp/ghostrace-0092-merged-lifecycle-native.log` | optimized FSEvents lifecycle receipt, exit 0 | `886fd1a00508698495b00ea1689f5ea94bdce6271908c2849a86d78b2904a7b3` | 1642 |
| `/tmp/ghostrace-0092-merged-offline.log` | offline canary/privacy pass; debug native no-go | `a6a57b1fc287cf2131059ab0141ec01fae69ce4a3a6ddade354f3cabbd6e1562` | 17611 |
| `fixtures/shell-wrapper-lifecycle-v1.json` | deterministic fixture registered in manifest | `0a7574897299f8e3af5cc258c7f13b2dab0ecd9146924ba54d0c9f9dacbdc827` | 1931 |

## Privacy, failure, and scope boundaries

- The reference harness runs fixed synthetic scripts only. It clears the child
  environment and connects stdin/stdout/stderr to null; it retains no command
  text, arguments, environment values, or terminal bytes.
- The fixture contains no raw command or process data. Its privacy flags and
  strict deserialization reject schema drift; gap rows carry only a typed reason.
- A timeout or cancellation returns the native signal in the completion contract;
  terminal closure, exec failure, and wrapper crash remain explicit gaps with no
  fabricated completion. Cleanup kills and waits for abandoned children.
- This task does not implement a shell executor, PTY, terminal-close detector,
  process attribution, or a production policy/consent gate. Those remain parent
  task 0024 work.
