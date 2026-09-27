# Task 0098 evidence: frontmost application identity and session semantics

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#350](https://github.com/AlisinaDevelo/GHOSTRACE/pull/350)
was squash-merged to protected `main` at `f5cab8c2a474b332f72eb1c8485736a4c843e1c0`.

The deliverable is the normalization contract in `src/frontmost.rs` with `schemas/frontmost-observation-v1.json` and `fixtures/frontmost-identity-v1.json`. No NSWorkspace collector is shipped.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0098-01 | The schema distinguishes bundle identifier, executable signing identity, launch instance, activation, deactivation, termination, and unknown app. | `FrontmostApp::Known` retains bundle ID, signing class (developer with team ID, platform, ad hoc, unsigned, unknown), kind, location, and a salted launch-instance digest; `FrontmostTransition` distinguishes activated, deactivated, and terminated; `FrontmostApp::Unknown` records why identity is absent. The schema is validated against every normalized output. |
| E-0098-02 | Window titles, document names, URLs, accessibility data, menu state, and screen contents are structurally absent. | `FrontmostRawObservation` has no field for them and strict parsing rejects seven hostile inputs without echoing their values (`titles_documents_urls_accessibility_menus_and_screens_are_rejected_without_echo`). |
| E-0098-03 | Unsigned, translocated, helper, command-line, and rapidly switching applications have tested outcomes. | The corpus has 14 identity cases including unsigned, translocated, accessory and prohibited helpers, and command-line tools, plus a rapid-switching session sequence with transient marking and duplicate suppression (`identity_cases_normalize_to_their_expected_outcomes`, `session_sequences_produce_bounded_dwell_and_transient_marks`). |

## Delivery

- Issue: [#102](https://github.com/AlisinaDevelo/GHOSTRACE/issues/102)
- Implementation PR: [#350](https://github.com/AlisinaDevelo/GHOSTRACE/pull/350)
- Implementation commit before squash: `0b4fa428493825388d6aa888a8561c0ee9cb96c0`
- Protected-main merge: `f5cab8c2a474b332f72eb1c8485736a4c843e1c0`
- Verification date: 2026-09-27 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 4b84b424f76b441b1b328330301b22961634390c
```

## Merged-main device verification

Every command ran from protected `main` at `4b84b424f76b441b1b328330301b22961634390c`, which contains this task's
merge. Hosted checks are corroboration; the retained device logs are the
acceptance evidence.

- `bash scripts/reproducibility-test.sh` exited `0` ("all checks passed").
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 300 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 51 tests.

Not run in this verification: the sandboxed `scripts/offline-network-test.sh`
lane and the native filesystem benchmark, whose resource no-go on this device is
tracked in task 0163 ([#348](https://github.com/AlisinaDevelo/GHOSTRACE/issues/348)).

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `frontmost_identity` 6/6.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-4b84b42/device-info.txt` | exact device/toolchain capture | `d721fd33fefaa221a0550111da4bcecec17d88b41782f1584af2ef365d943d79` | 230 |
| `ghostrace-evidence/merged-4b84b42/repro.log` | reproducibility lane, exit 0 | `3691ebb1b5bdbc66629b7e2da733389532bb235edbb9c83035b1cf67ba455fce` | 44792 |
| `ghostrace-evidence/merged-4b84b42/clippy.log` | Clippy with warnings denied, exit 0 | `1c8d1f786821918fdcf1365da4e2279f678760ad63fee273a38ff82977f19ffb` | 60 |
| `ghostrace-evidence/merged-4b84b42/all-targets.log` | all targets except native benchmark, 300 passed | `c432cda2ec58d0eca3b2eba9deb82daaad9cab47ea992f61992c1b58ce31e8ca` | 37430 |
| `ghostrace-evidence/merged-4b84b42/rustdoc.log` | rustdoc with warnings denied, exit 0 | `d2f00845cc6eeea27da7b254f9187086de90f3b9503cd78eec5e89027bf79ee4` | 656 |
| `ghostrace-evidence/merged-4b84b42/release-build.log` | optimized release build, exit 0 | `a9c9a28f560875c357dbb15ee4e7225b97c5688a3129fe05f564afe4d9249b74` | 3438 |
| `ghostrace-evidence/merged-4b84b42/python.log` | Python suite, 51/51 | `568f4fec8b5c5d30958b68d7a2c4f7ba9726fd9e513ac75b14998c2096d7c852` | 150 |
| `ghostrace-evidence/merged-4b84b42/release-frontmost_identity.log` | optimized frontmost_identity suite, 6/6 | `e33e042f18fcea8c04c01c07e2396379574397bb3b98247adef3eadcf223863c` | 842 |

## Privacy, failure, and scope boundaries

- All inputs are synthetic; launch instances are salted digests and never expose the process ID (`launch_instances_are_salted_and_never_expose_the_process_id`).
- Coverage across sleep, lock, and observer restarts is task 0099.
