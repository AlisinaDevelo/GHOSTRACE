# Task 0022 evidence: optional Parquet cold-archive export

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation: [#390](https://github.com/AlisinaDevelo/GHOSTRACE/pull/390) (squash-merged at `4f26e31ecfba1750f13afda56e4d20ef858ac92b`).

The deliverable is `src/parquet_archive.rs` behind the opt-in `parquet` feature, with `ghostrace archive` and `ghostrace verify-archive`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0022-01 | Parquet schema metadata and checksums are documented. | `docs/ARCHITECTURE.md` documents the physical types, row-group bounds, and the `ghostrace.archive.*` footer keys: profile identity and digest, source manifest and event-body digests, row count, and the canonical-rows digest. |
| E-0022-02 | JSONL and Parquet representations compare successfully. | Before publication every row is read back, rebuilt into its JSONL record, and compared with the export through `EventEnvelope`; a mismatched export and a damaged file are rejected (`tests/parquet_archive.rs` 4/4 optimized). DuckDB v1.5.5 independently read the archive built on merged main: 8 rows, 1 gap row, UTC timestamps, unsigned sequence (`device-checks.log`). |
| E-0022-03 | Archive creation is explicit and never automatic. | The writer exists only in `--features parquet` builds and only through `ghostrace archive`; it never replaces an existing file. |
| E-0022-04 | The command warns about plaintext disclosure. | `ghostrace archive` prints the plaintext warning and refuses without `--yes`; the warning is also stored in the footer (`cli_requires_explicit_confirmation_and_warns_about_plaintext`). |

## Delivery

- Issue: [#26](https://github.com/AlisinaDevelo/GHOSTRACE/issues/26)
- Implementation PR: [#390](https://github.com/AlisinaDevelo/GHOSTRACE/pull/390), head before squash `6df9bc566054bd54a76f11eec0fa839ee73b8a2e`, merge `4f26e31ecfba1750f13afda56e4d20ef858ac92b`
- Verification date: 2026-09-29 UTC

## Device and toolchain

```text
Darwin 25.6.0
26.6.2
MacBookPro17,1
Apple M1
8
arm64
rustc 1.88.0 (6b00bc388 2025-06-23)
host: aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 8fdac72cbf53c82f7ec0634b9aa2d53b02cde17e
```

## Merged-main device verification

Every command ran from protected `main` at `8fdac72cbf53c82f7ec0634b9aa2d53b02cde17e`, which contains this task's
merge. Hosted checks are corroboration; the retained device logs are the
acceptance evidence.

- `bash scripts/reproducibility-test.sh` exited `0`.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 375 passed, 0 failed, 3 ignored. It ran with a private
  target directory; a first run that shared its target directory with a concurrent
  default-feature build failed one Parquet CLI test and is retained as
  `all-targets-shared-target-contaminated.log`.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 68 tests.

Focused optimized run `cargo +1.88.0 test --release --locked --all-features --test parquet_archive -- --nocapture` exited `0`: 4/4.

`device-checks.sh` built, verified, and read an archive with DuckDB; see `device-checks.log`.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-8fdac72/device-info.txt` | exact device/toolchain capture | `bbc5bb3c38651078cd101bfb4efbcb8bd15475e6f1d1ff5b8c0f0c4362b17558` | 232 |
| `ghostrace-evidence/merged-8fdac72/repro.log` | reproducibility lane, exit 0 | `584e58e8e14138a1272d694ffa11e29f50260a5500e4bd96e32325bc31d884cb` | 54217 |
| `ghostrace-evidence/merged-8fdac72/clippy.log` | Clippy with warnings denied, exit 0 | `a0c8a44b73e7d1b71117f801e8821241cba722b97d85c765768ee9108994929b` | 146 |
| `ghostrace-evidence/merged-8fdac72/all-targets.log` | all targets except native benchmark, 375 passed | `1c01d0c20beeafb43f9a80601663568ff5d02a7a5e055c8b4ab1a6cd47a25d06` | 56512 |
| `ghostrace-evidence/merged-8fdac72/rustdoc.log` | rustdoc with warnings denied, exit 0 | `d241e86b5c2d424451d49d9a0d55cc4e3323c058db82b5d7b4031b8ba69bc8aa` | 349 |
| `ghostrace-evidence/merged-8fdac72/release-build.log` | optimized release build, exit 0 | `28b252d1d9f56ce3c52e0dc995d00d4d4df450fee4f493a024eefac75f242fe8` | 3439 |
| `ghostrace-evidence/merged-8fdac72/python.log` | Python suite, 68/68 | `16cad5d27dae182c999b203dcad169659b4acb409c372389821e39dcf1af2c4e` | 167 |
| `ghostrace-evidence/merged-8fdac72/all-targets-shared-target-contaminated.log` | first run, shared target directory, 1 Parquet CLI failure from a concurrent build | `4613fcb543353b91f1125eedfc5bf0a8877a350b4e0fae6294340c7496ce9bf6` | 30161 |
| `ghostrace-evidence/merged-8fdac72/release-parquet_archive.log` | optimized parquet_archive suite, 4/4 | `39e207c296460371e0832314d15fffd7a837e189958ca0f79a85d0ea14483990` | 2195 |
| `ghostrace-evidence/merged-8fdac72/release-build-all-features.log` | optimized --all-features build, exit 0 | `7523be007a23bc74729002ed258238b7049293936c7865d09d089ddc5d4e9427` | 3955 |
| `ghostrace-evidence/merged-8fdac72/device-checks.sh` | device check script | `35c66d393b4e61cfde73da8a8dfd4f597c5ba6198e132303289c7a2388736983` | 3170 |
| `ghostrace-evidence/merged-8fdac72/device-checks.log` | device checks including DuckDB read, exit 0 | `9cd09048f64c667833d23e9a11399d6207ea2d0edf18b5bd4842e222aaba5300` | 2635 |

## Privacy, failure, and scope boundaries

- The archive is plaintext by design and is not removed by journal retention or key destruction.
- It is written from a validated JSONL export, never from the journal directly; archiving a live journal is task 0173 (#394).
