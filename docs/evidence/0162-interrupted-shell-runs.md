# Task 0162 evidence: interrupted shell runs closed with explicit gaps

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#351](https://github.com/AlisinaDevelo/GHOSTRACE/pull/351)
was squash-merged to protected `main` at `d4c0bb4d4a8175ea20e80f9cfa2e534ea8e0edc3`.

The deliverable is SIGTERM/SIGHUP forwarding, `ShellWrapper::recover_incomplete_runs`, and the incomplete-run explanation warning in `src/shell_wrapper.rs` and `src/explain.rs`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0162-01 | SIGTERM and SIGHUP delivered to the wrapper are forwarded to the child, and the wrapper records a terminal gap when it cannot observe the child's status. | A helper process re-executes the test binary as a wrapper whose child signals it: SIGTERM and SIGHUP are forwarded, the child's signaled outcome is journaled (signal 15 or 1), and the wrapper exits 143 or 129 (`sigterm_to_the_wrapper_is_forwarded_and_the_outcome_recorded`, `terminal_close_is_forwarded_and_the_outcome_recorded`). A wait failure or crash leaves no fabricated status; recovery closes the run with a `shell_run_incomplete` gap. |
| E-0162-02 | Query coverage and explanations report a `shell_started` without a terminal event as an incomplete run with no end time or success status. | `explain` warns "shell run has no terminal observation" for a wrapper start with no finish or gap child, and makes no success or finish statement (`crashed_wrapper_leaves_an_incomplete_run_that_recovery_closes`); completed runs carry no warning (`completed_runs_are_not_reported_as_incomplete`). |
| E-0162-03 | Tests cover terminal close, wrapper termination, and journal recovery after a wrapper crash. | Terminal close (SIGHUP), wrapper termination (SIGTERM), and a SIGABRT crash against a file-backed journal are each exercised; recovery respects its age bound, closes the dangling run once, and is idempotent. |

## Delivery

- Issue: [#347](https://github.com/AlisinaDevelo/GHOSTRACE/issues/347)
- Implementation PR: [#351](https://github.com/AlisinaDevelo/GHOSTRACE/pull/351)
- Implementation commit before squash: `0af6caa360e487ae72de0edff3f3b3df0a5cad9a`
- Protected-main merge: `d4c0bb4d4a8175ea20e80f9cfa2e534ea8e0edc3`
- Verification date: 2026-09-27 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 5a0bdbba25a97009d3bf520b90b83198e65da6ec
```

## Merged-main device verification

Every command ran from protected `main` at `5a0bdbba25a97009d3bf520b90b83198e65da6ec`, which contains this task's
merge. Hosted checks are corroboration; the retained device logs are the
acceptance evidence.

- `bash scripts/reproducibility-test.sh` exited `0`.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 315 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 61 tests.

Not run in this verification: the sandboxed `scripts/offline-network-test.sh`
lane and the native filesystem benchmark (task 0163,
[#348](https://github.com/AlisinaDevelo/GHOSTRACE/issues/348)).

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `shell_interrupted_runs` 5/5; `shell_run_wrapper` 10/10.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-5a0bdbb/device-info.txt` | exact device/toolchain capture | `7798fb73727694248eb983dbfdc23d0fd45969f65043be307f36890dc9d9396f` | 230 |
| `ghostrace-evidence/merged-5a0bdbb/repro.log` | reproducibility lane, exit 0 | `bf44669f553395a8875f917d34e45e3e711879f425fd77860c5426cd9fa3ecb0` | 46817 |
| `ghostrace-evidence/merged-5a0bdbb/clippy.log` | Clippy with warnings denied, exit 0 | `ef63b9eaa3ff5c9cf8b10629df2d4980ea6116664128b6824a0689a2c845252d` | 114 |
| `ghostrace-evidence/merged-5a0bdbb/all-targets.log` | all targets except native benchmark, 315 passed | `0e75ac2f1ccb114916f5023ecd5092c367dfcc67dc7e97de2142b8c7f1969392` | 39365 |
| `ghostrace-evidence/merged-5a0bdbb/rustdoc.log` | rustdoc with warnings denied, exit 0 | `6b2e23b65ef3da4c3ee11308c00f8816f00feadbd198a154774a82492236cc42` | 234 |
| `ghostrace-evidence/merged-5a0bdbb/release-build.log` | optimized release build, exit 0 | `249362d739022f07f59584f28dc29213827061c19d6061111e666d42ea82c402` | 147 |
| `ghostrace-evidence/merged-5a0bdbb/python.log` | Python suite, 61/61 | `805bae5666c4dae6ebfa7f851fa092f8371ec0913d06e4598a8e1e5191ad6a02` | 160 |
| `ghostrace-evidence/merged-5a0bdbb/release-shell_interrupted_runs.log` | optimized shell_interrupted_runs suite, 5/5 | `b6b06a29f9e5ed70702123a99fe18dc074663e22d7ef352d4e781aa9ca3bd440` | 747 |
| `ghostrace-evidence/merged-5a0bdbb/release-shell_run_wrapper.log` | optimized shell_run_wrapper suite, 10/10 | `f03670993cf9730b3d91a7147557f78d009a4cd66e6fc6c8681e403dcba3c2cd` | 1105 |

## Privacy, failure, and scope boundaries

- Recovery is age-bounded so a run still live in another process is not closed.
- Signal dispositions are installed only after the child is spawned and restored afterwards; runs are serialized per process.
