# Task 0164 evidence: opt-in login-keychain key custody

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation: [#387](https://github.com/AlisinaDevelo/GHOSTRACE/pull/387) (squash-merged at `d3b6800998f8c251df1990a77e8d2fd63fce4226`).

The deliverable is `KeyCustody` and the login-keychain constructors of `MacOsKeychainProvider` in `src/keychain.rs`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0164-01 | The login-keychain backend is selected only by an explicit flag or configuration and is reported as such by status output. | Only `MacOsKeychainProvider::login_keychain*` selects it; the default constructor stays on the data-protection keychain (`custody_is_explicit_and_data_protection_is_the_default`). `ghostrace live init` records `custody: login_keychain` in the home's config, and `live status` reports it (`key custody: LoginKeychain` in `device-checks.log`; `status["custody"] == "login_keychain"` in `live_cli`). |
| E-0164-02 | The key item is non-synchronizable, created only by an explicit provisioning step, and never read or created implicitly. | The item is a generic password in the local login keychain, which is not an iCloud-synchronized keychain. It is created only by `provision` (called by `live init`), which refuses to overwrite; reads never create an item (`login_keychain_round_trips_a_journal_key`: provision, read back, refused overwrite, encrypted journal reopened, delete). `live forget` deletes it. |
| E-0164-03 | Documentation states the weaker binding (the item's access list follows the binary's signature, so rebuilds prompt) and the unchanged encryption format. | `docs/PRIVACY.md`, section "Key custody without Developer ID signing", and `docs/DEMO.md`. The follow-up to remove the rebuild prompt without a Developer ID is task 0172 (#393). |

## Delivery

- Issue: [#379](https://github.com/AlisinaDevelo/GHOSTRACE/issues/379)
- Implementation PR: [#387](https://github.com/AlisinaDevelo/GHOSTRACE/pull/387), head before squash `2cd6281ecee12da570b0bd712febed13216779d4`, merge `d3b6800998f8c251df1990a77e8d2fd63fce4226`
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

These logs were lost to a reboot; see the re-verification below.

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

- The login keychain is weaker than the data-protection keychain: another process running as the user that the user approves in the access prompt can read the key.
- Evidence covers the reference M1 only.

## Re-verification after log loss

The logs listed above were kept in the device's temporary directory, and a reboot on
2026-09-29 deleted them. The task was verified again on protected `main` at
`00e0dc773b13daa294b9397105c43b81d8f3df2d`, which still contains its merge. Those logs are kept in
`~/Developer/dev/ghostrace-evidence`, outside temporary storage.

Every command ran from protected `main` at `00e0dc773b13daa294b9397105c43b81d8f3df2d` on the reference device, with
a private Cargo target directory and `GIT_CONFIG_GLOBAL=/dev/null`. Hosted checks are
corroboration; the retained device logs are the acceptance evidence.

- `bash scripts/reproducibility-test.sh`, `cargo +1.88.0 clippy --locked --all-targets
  --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked
  --no-deps`, and `cargo +1.88.0 build --release --locked` (with and without
  `--all-features`) exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited `0`:
  382 passed, 0 failed, 3 ignored.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 73 tests.
- `GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 cargo +1.88.0 test --release --locked --all-features
  --test login_keychain --test live_cli` exited `0`: 2/2 and 2/2.
- `GHOSTRACE_FRONTMOST_TEST=1 cargo +1.88.0 test --release --locked --features frontmost
  --test frontmost_macos --test frontmost_identity` exited `0`.
- `device-checks.sh` and `scripts/live-demo.sh` ran against the optimized
  `--all-features` binary and exited `0`. A first device-check run used a binary that a
  later feature-specific test build had replaced; it stopped at the archive step and is
  retained under its own name.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-00e0dc7/device-info.txt` | exact device/toolchain capture | `b7723aa8216b7b867cf2402518c0106f93c330786ea76c51e6579e84a022ca34` | 232 |
| `ghostrace-evidence/merged-00e0dc7/repro.log` | reproducibility lane, exit 0 | `38820862cdc6d183786f9e17d2b523a743df5cde56c77a7127b5ff7e6dc042e5` | 61621 |
| `ghostrace-evidence/merged-00e0dc7/clippy.log` | Clippy with warnings denied, exit 0 | `2e50eba1ed57adc5f3d07f377702d6e438a23ebd1b3d424eb36c6677a9d323a9` | 60 |
| `ghostrace-evidence/merged-00e0dc7/all-targets.log` | all targets except native benchmark, 382 passed | `d8983ca62f19352820196582152066e2abb1355a0cf579d0cf3fe31f2e54f8fb` | 48122 |
| `ghostrace-evidence/merged-00e0dc7/rustdoc.log` | rustdoc with warnings denied, exit 0 | `0038e008d6fcdfd95721afdd77def13671f6bb396e0a0cb2dc788b55d347be7e` | 1055 |
| `ghostrace-evidence/merged-00e0dc7/release-build.log` | optimized release build, exit 0 | `07af3cf1276fc49ba3baec095e7ea24bef7afc3ce4d4eefe3b21c5809a4a0852` | 3439 |
| `ghostrace-evidence/merged-00e0dc7/release-build-all-features-rebuild.log` | optimized --all-features build used by the scripts, exit 0 | `d1235ea4563620b53ef5bdd2da5f595fec9d54f9efb48376378c20547dd95e57` | 62 |
| `ghostrace-evidence/merged-00e0dc7/python.log` | Python suite, 73/73 | `143fd0af1bf19eb57cbe2d5f43a62aefd7100fc91d2ffbd14b11401cb97c2ca2` | 172 |
| `ghostrace-evidence/merged-00e0dc7/release-keychain-live.log` | gated live_cli 2/2 and login_keychain 2/2 | `49162e77c6372c58eff9b31ab3c152ee80c4c0e62a5bfc30592055350e84ff88` | 2629 |
| `ghostrace-evidence/merged-00e0dc7/release-focused.log` | authenticated_state 9/9, parquet_archive 4/4, timeline_report 3/3 | `04ea7241658c9584d63314b5263db25dd748f8777ac0e14a98d88fee6f4e46e7` | 2040 |
| `ghostrace-evidence/merged-00e0dc7/release-frontmost.log` | frontmost_identity 8/8 and the frontmost_macos checks including a real focus switch | `430554f87cbbc13102a3057e3afe6fa84f4ba2249ba41b46bd8c2082c7a98b5e` | 1806 |
| `ghostrace-evidence/merged-00e0dc7/device-checks.sh` | device check script | `3e6972848ae88a3be55a5a6c510e959ee3a7df42d2d4dbcf5a5824c30c49c760` | 3394 |
| `ghostrace-evidence/merged-00e0dc7/device-checks.log` | device checks, exit 0 | `5f683ddd2096a29e9f171e40df94d58a5c506da7c920cdf42004fa035130eb2c` | 2789 |
| `ghostrace-evidence/merged-00e0dc7/device-checks-frontmost-only-binary.log` | first device-check run, against a binary rebuilt without the parquet feature; exit 1 at the archive step | `904b608a3476e3bc34014e534c78ee7742446b55cf9a065b60fd808f0d728546` | 2848 |
| `ghostrace-evidence/merged-00e0dc7/live-demo.log` | scripts/live-demo.sh, exit 0 | `ab55bca27e41990d7cf37b980f7a9b6707fc935859fbc2da819c695a9c3a0d0b` | 8308 |
