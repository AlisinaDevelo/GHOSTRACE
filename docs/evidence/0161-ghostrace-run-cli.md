# Task 0161 evidence: explicit ghostrace run CLI over a Keychain journal

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation: [#388](https://github.com/AlisinaDevelo/GHOSTRACE/pull/388) (squash-merged at `fbaffc57298fc744c74e34147b7bd0aaff9ab7c6`), [#391](https://github.com/AlisinaDevelo/GHOSTRACE/pull/391) (squash-merged at `c7f759e915a2ff53fc78b43c4e5fd9d8b6ef7e51`).

The deliverable is `ghostrace run` (`LiveHome::run`) with persisted consent (`live consent-shell`, `live revoke-shell`).

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0161-01 | The command refuses to run until a persisted consent receipt for a shell-enabled policy exists, and `ghostrace run` never becomes ambient capture. | The consent receipt is stored in the private config, bound to the SHA-256 of the exact preview accepted; without it `run` fails before spawning (`live_cli`: a `touch` marker is never created; `device-checks.log`: exit 1 with the consent message). Only commands started through `ghostrace run` are recorded. |
| E-0161-02 | The durable journal uses Keychain key custody (data-protection when signed, or the explicit login-keychain option from 0164); the fixture seed is never used for a live journal. | The live journal is opened only with the home's login-keychain provider (0164); no fixture key provider is reachable from `LiveHome`. |
| E-0161-03 | The command exits with the child's status, prints nothing derived from arguments, environment, or terminal streams, and a revoked consent refuses before spawning. | Exit 7 and 127 pass through (`live_cli`, `device-checks.log`). The wrapper records only metadata: the sentinel argument never appears in the timeline (`live_cli`) or in the timeline, status, or any explanation (`device-checks.log`), and environment and terminal streams are covered by the shell wrapper's secret-leakage corpus from task 0024. After `live revoke-shell` the next run refuses and its marker file is not created (`live_cli`). |

## Delivery

- Issue: [#346](https://github.com/AlisinaDevelo/GHOSTRACE/issues/346)
- Implementation PR: [#388](https://github.com/AlisinaDevelo/GHOSTRACE/pull/388), head before squash `7725efb360568425b34306b84c07e62ac0a9c873`, merge `fbaffc57298fc744c74e34147b7bd0aaff9ab7c6`
- Implementation PR: [#391](https://github.com/AlisinaDevelo/GHOSTRACE/pull/391), head before squash `be66870429da547e28e2cc04296c0c59762afa2a`, merge `c7f759e915a2ff53fc78b43c4e5fd9d8b6ef7e51`
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

Gated login-keychain runs, `GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 cargo +1.88.0 test --release --locked --test login_keychain --test live_cli -- --nocapture`, exited `0`: `live_cli` 2/2 and `login_keychain` 2/2.

`device-checks.sh` ran against the optimized `--all-features` build (`cargo +1.88.0 build --release --locked --all-features`, exit `0`) in a throwaway home and exited `0`; `device-checks.log` holds its complete output.

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
| `ghostrace-evidence/merged-8fdac72/release-keychain-live.log` | gated live_cli 2/2 and login_keychain 2/2 | `11a7abccbf667b1df0dd0fc3c5b0fec10cec932c6cc4c42e0ead2094acbcf629` | 2620 |
| `ghostrace-evidence/merged-8fdac72/release-build-all-features.log` | optimized --all-features build, exit 0 | `7523be007a23bc74729002ed258238b7049293936c7865d09d089ddc5d4e9427` | 3955 |
| `ghostrace-evidence/merged-8fdac72/device-checks.sh` | device check script | `35c66d393b4e61cfde73da8a8dfd4f597c5ba6198e132303289c7a2388736983` | 3170 |
| `ghostrace-evidence/merged-8fdac72/device-checks.log` | device checks, exit 0 | `9cd09048f64c667833d23e9a11399d6207ea2d0edf18b5bd4842e222aaba5300` | 2635 |

## Privacy, failure, and scope boundaries

- A program that cannot be started is recorded as a `shell_exec_failed` gap and exits 127.
- Interrupted runs are task 0162's evidence.
