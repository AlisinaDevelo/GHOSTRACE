# Task 0121 evidence: schema and export compatibility matrix

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#360](https://github.com/AlisinaDevelo/GHOSTRACE/pull/360)
was squash-merged to protected `main` at `47645a616ab7fb178d424a24b09e3aeaf00bbc94`.

The deliverable is `planning/compatibility-matrix.json`, `scripts/compatibility.py`, `tests/compatibility_matrix.rs`, and `docs/COMPATIBILITY.md`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0121-01 | Golden artifacts from every supported version run through upgrade, query, explanation, export, verification, and deletion paths. | Every retained format is v1 (the journal database accepts migration 1 through 5). `legacy_journal_survives_upgrade_query_explain_export_verify_and_delete` creates a journal at migration 1, upgrades it, ingests the golden causal-chain fixture, queries, explains, exports and validates the export, runs the integrity check, deletes through confirmed retention, and reopens it. The v1 JSON contracts accept their goldens. |
| E-0121-02 | Forward, backward, unknown, mixed, corrupted, and partially migrated cases have explicit accept or refuse outcomes. | The matrix states an outcome for each case per format and names its proving test (29 proven cases across 5 formats); `scripts/compatibility.py check` runs in the required roadmap CI job and fails if a named test does not exist or an unsafe case is accepted. |
| E-0121-03 | Removing compatibility requires a deprecation window, migration tool, rollback evidence, and release-note impact statement. | The checker refuses a deprecated format or retired version without `announced_in`, `removal_not_before`, an existing migration tool, existing rollback evidence, and a release note, and refuses a retired version still listed as readable (`tests/test_compatibility.py`). |

## Delivery

- Issue: [#125](https://github.com/AlisinaDevelo/GHOSTRACE/issues/125)
- Implementation PR: [#360](https://github.com/AlisinaDevelo/GHOSTRACE/pull/360)
- Implementation commit before squash: `905416658b6949b05bb9cf72273b38d5d6039c73`
- Protected-main merge: `47645a616ab7fb178d424a24b09e3aeaf00bbc94`
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
-- --nocapture`, exited `0`: `compatibility_matrix` 6/6.

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
| `ghostrace-evidence/merged-ce647cb/release-compatibility_matrix.log` | optimized compatibility_matrix suite, 6/6 | `0bbaf3fcd1608a74e68a170cefa22653cdaa47bf443e39e2ae649903d53c0005` | 890 |

## Privacy, failure, and scope boundaries

- Only v1 formats exist, so backward compatibility for JSON contracts is refusal of version 0; the database is the only format with upgrades.
- Checks run offline on synthetic fixtures.
