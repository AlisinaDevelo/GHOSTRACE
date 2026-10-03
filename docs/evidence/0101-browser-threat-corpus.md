# Task 0101 evidence: browser integration threat corpus

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#363](https://github.com/AlisinaDevelo/GHOSTRACE/pull/363)
was squash-merged to protected `main` at `3c1d035b8451ae08cb429c1d6267e46ca4eccc00`.

The deliverable is `fixtures/browser-threat-corpus-v1.json`, `tests/browser_threat_corpus.rs`, and the risk register in ADR 0005.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0101-01 | The corpus covers spoofed origins, oversized frames, duplicate and replayed messages, Unicode and URL confusion, extension replacement, and downgrade attempts. | 32 cases across all eight categories; `corpus_covers_every_required_threat_category_and_names_a_layer_and_outcome` requires every category. |
| E-0101-02 | Every case states which layer validates, rejects, records a gap, or requires re-pairing. | Each case names one of seven layers and one of four outcomes. The 17 URL cases are enforced against the shipped `SanitizedUrl` with exact canonical output; the remaining cases name the ledger task that must enforce them, and the test fails if that task is missing. |
| E-0101-03 | The security ADR links each accepted risk to a test, permission, user control, and rollback path. | `every_accepted_risk_links_a_test_permission_user_control_and_rollback` parses ADR 0005's accepted-risk table, requires all six columns for every risk, requires each test to be a corpus case, and requires every case's `accepted_risk` to appear in the ADR. |

## Delivery

- Issue: [#105](https://github.com/AlisinaDevelo/GHOSTRACE/issues/105)
- Implementation PR: [#363](https://github.com/AlisinaDevelo/GHOSTRACE/pull/363)
- Implementation commit before squash: `91b9bdccf681c940496743b74b072fc5f2161274`
- Protected-main merge: `3c1d035b8451ae08cb429c1d6267e46ca4eccc00`
- Verification date: 2026-09-28 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: ce647cb25e855d669f55b44f414d25851cb3b7a0
```

## Merged-main device verification

Every command ran from protected `main` at `ce647cb25e855d669f55b44f414d25851cb3b7a0`, which contains this task's
merge, unless stated otherwise below. Hosted checks are corroboration; the
retained device logs are the acceptance evidence.

- `bash scripts/reproducibility-test.sh` exited `0`.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 335 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 68 tests.
- `bash scripts/offline-network-test.sh` (sandbox-exec `deny network*`) exited
  `0`: canary, privacy fixture, and the complete product suite including the
  native filesystem benchmark, 338 passed, 0 failed, 3 ignored. The load average
  was 29 at the start of that run with 7.3 GB of swap in use.

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `browser_threat_corpus` 4/4.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-ce647cb/device-info.txt` | exact device/toolchain capture | `0074eaf20d22e1d4ab5fee044109ff1711331d1a0b30eec8abb768eddc30cf3e` | 230 |
| `ghostrace-evidence/merged-ce647cb/repro.log` | reproducibility lane, exit 0 | `ebfd5e6b8887efd8deef8a56407f4930b8fd6221d1f6398da133c16a6ef89bf0` | 49791 |
| `ghostrace-evidence/merged-ce647cb/clippy.log` | Clippy with warnings denied, exit 0 | `b2f631e548357db6db995dbe5f9a01a397a0e223799756409e06429df755fab1` | 60 |
| `ghostrace-evidence/merged-ce647cb/all-targets.log` | all targets except native benchmark, 335 passed | `14f0a99b8db17bda432f140e994b04da48109dacdd3e1c5da929ca6af93aa1cf` | 41803 |
| `ghostrace-evidence/merged-ce647cb/rustdoc.log` | rustdoc with warnings denied, exit 0 | `6729040597c83b0eeb37e25053addcdf65892061971b4d5c8b8ae526a48bba7c` | 234 |
| `ghostrace-evidence/merged-ce647cb/release-build.log` | optimized release build, exit 0 | `21ebac8bfb8ce88972517be90824dc1970a6e6d4cc3d4d11ae7f84f1b0de96a7` | 147 |
| `ghostrace-evidence/merged-ce647cb/python.log` | Python suite, 68/68 | `1a9a52815cb0b7353e31be22f3dc2059ae76eb265a49a7f81ceae82b473c3670` | 167 |
| `ghostrace-evidence/merged-ce647cb/offline.log` | offline network-denial lane with native benchmark, 338 passed | `4f40fe36395494da4468b6eef5efdfc7c15576e4ae87ac08bcf0c83a310ba253` | 42983 |
| `ghostrace-evidence/merged-ce647cb/offline-machine.txt` | load and swap around the offline lane | `9cfe4b6accc1ea6210b016761ae14509e56b30bef8733ee8099b79d7a6e3a1c2` | 213 |
| `ghostrace-evidence/merged-ce647cb/release-browser_threat_corpus.log` | optimized browser_threat_corpus suite, 4/4 | `e74a8117e0dcfcedf7d977a8d861f918e071a97bfef74e79ef2445d3b82a4611` | 732 |

## Privacy, failure, and scope boundaries

- The corpus is synthetic. The separate canonical navigation shape introduced in #364 addresses its path, trailing-dot and private-host findings for future navigation collection. The legacy v1 `SanitizedUrl` fixture/event type still has those limitations, so its corpus findings remain open; no browser collector is enabled. [The origin-digest correction](0106-browser-origin-boundary.md) records the scheme boundary and retained-origin limits.
