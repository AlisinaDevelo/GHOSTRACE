# Task 0099 evidence: frontmost sleep, wake, and privacy transitions

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#357](https://github.com/AlisinaDevelo/GHOSTRACE/pull/357)
was squash-merged to protected `main` at `3c5fdc595baf0b3d46a11f79db4d60f680c68893`.

The deliverable is the coverage-aware `FrontmostSessionTracker` in `src/frontmost.rs`, `schemas/frontmost-coverage-v1.json`, and `fixtures/frontmost-coverage-v1.json`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0099-01 | A state-machine corpus identifies which transitions are direct notifications, inferred closures, or gaps. | The corpus transition table classifies startup, login, fast user switching, lock, sleep, wake, Mission Control, termination, missed deactivation, and observer restart, and 12 sequences pin exact records; every app record carries `basis` (direct or inferred_closure) and coverage boundaries mark suspended, resumed, and interrupted intervals (`corpus_classifies_every_required_transition`, `every_sequence_produces_exactly_the_expected_records`). |
| E-0099-02 | Private applications and user exclusions are filtered before persistence. | `FrontmostExclusions` replaces excluded bundle identifiers with an `excluded` unknown app before any record exists (`excluded_applications_keep_no_identity_in_any_record`). |
| E-0099-03 | Missed notifications and observer downtime never extend a prior app session as if coverage were continuous. | An activation closes an open session at its own time; suspensions close at the boundary; an unclean observer restart drops the session without a dwell. `no_session_dwell_spans_a_suspension_or_interruption` checks that no dwell interval contains a suspension or interruption across every sequence. |

## Delivery

- Issue: [#103](https://github.com/AlisinaDevelo/GHOSTRACE/issues/103)
- Implementation PR: [#357](https://github.com/AlisinaDevelo/GHOSTRACE/pull/357)
- Implementation commit before squash: `d2bb6ab2098aa09c1343fcd0835fe7a6fd72329c`
- Protected-main merge: `3c5fdc595baf0b3d46a11f79db4d60f680c68893`
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
-- --nocapture`, exited `0`: `frontmost_coverage` 4/4; `frontmost_identity` 6/6.

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
| `ghostrace-evidence/merged-5a0bdbb/release-frontmost_coverage.log` | optimized frontmost_coverage suite, 4/4 | `e58ae75bfc42ba943e3cd0f9dd5e0345fb0600d51d6e6d74425bfbab43a57756` | 654 |
| `ghostrace-evidence/merged-5a0bdbb/release-frontmost_identity.log` | optimized frontmost_identity suite, 6/6 | `335ba7cdb2e60082c79650cc35fbede9a02458543aaf9060d3c59ef0461066de` | 843 |

## Privacy, failure, and scope boundaries

- All inputs are synthetic; no NSWorkspace collector is shipped.
- Records validate against their schemas in every sequence.
