# Task 0102 evidence: native-host manifest installation

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#374](https://github.com/AlisinaDevelo/GHOSTRACE/pull/374)
was squash-merged to protected `main` at `8652853d3425060ce1f31750ac115171482412fa`.

The deliverable is `NativeHostInstaller` in `src/native_host_manifest.rs`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0102-01 | Plan, install, verify, upgrade, disable, and uninstall preserve unrelated manifests and reject unsafe ownership, modes, links, and paths. | `plan` reports the exact file and action without writing; install exclusive-creates or atomically replaces only a receipt-matching manifest; moving the host binary is an upgrade; uninstall is the disable path, because removing the manifest is how a browser stops launching a host. An unrelated manifest survives install and uninstall; symlinked or group/other-writable directories anywhere below the support root, and linked or foreign-owned manifests, are refused (`plan_install_verify_and_uninstall_are_exact_and_idempotent`, `unrelated_manifests_are_preserved_and_foreign_ones_refused`, `unsafe_directories_links_and_modes_are_refused`, `moving_the_host_binary_is_an_upgrade_of_an_intact_manifest`). |
| E-0102-02 | Allowed extension origins are exact identifiers and wildcard or unknown origins are refused. | Only `chrome-extension://<32 letters a-p>/` is accepted and exactly one origin is written; wildcards, wrong length or alphabet, and upper case are refused before any write (`origins_must_be_exact_extension_identifiers`). |
| E-0102-03 | Uninstall removes only a manifest whose digest and installation receipt still match. | Uninstall compares the manifest to its receipt digest; a manifest edited to admit another origin is reported as drift and survives uninstall (`a_manifest_edited_to_admit_another_origin_is_drift_and_never_removed`). |

## Delivery

- Issue: [#106](https://github.com/AlisinaDevelo/GHOSTRACE/issues/106)
- Implementation PR: [#374](https://github.com/AlisinaDevelo/GHOSTRACE/pull/374)
- Implementation commit before squash: `03a05152c919424728d5926413f343b0cf49694f`
- Protected-main merge: `8652853d3425060ce1f31750ac115171482412fa`
- Verification date: 2026-09-28 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 37ff5103ff69193db1f284967481db594b51ac90
```

## Merged-main device verification

Every command ran from protected `main` at `37ff5103ff69193db1f284967481db594b51ac90`, which contains this task's
merge. Hosted checks are corroboration; the retained device logs are the
acceptance evidence.

- `bash scripts/reproducibility-test.sh` exited `0`.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 366 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 68 tests.

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `native_host_manifest` 7/7.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-37ff510/device-info.txt` | exact device/toolchain capture | `f4477aa00dcfa3399f8b3bda435e6f1ffcfedebdb7f8b6fb84e08b84a040b0f5` | 230 |
| `ghostrace-evidence/merged-37ff510/repro.log` | reproducibility lane, exit 0 | `3748a63992de5feacbdff98f48d72ccedf5b15db7341333d5bdbda363baade00` | 52902 |
| `ghostrace-evidence/merged-37ff510/clippy.log` | Clippy with warnings denied, exit 0 | `3b2f45d0bb6c14a3a6681b691fd29d353102d9013db31b57d94895eee13eb8ca` | 60 |
| `ghostrace-evidence/merged-37ff510/all-targets.log` | all targets except native benchmark, 366 passed | `d63bf44629120fdc941078362765953fe98f0e746ddb24d8836654eba423c27e` | 45122 |
| `ghostrace-evidence/merged-37ff510/rustdoc.log` | rustdoc with warnings denied, exit 0 | `b7cc20566606a391567d909b22e908c88abba3f9789f465ff1ffcabcdb900367` | 234 |
| `ghostrace-evidence/merged-37ff510/release-build.log` | optimized release build, exit 0 | `67dee3ec9c250cede264a47341f05281f5fa9c07ddf6ce5d4152dd617c6f1b08` | 147 |
| `ghostrace-evidence/merged-37ff510/python.log` | Python suite, 68/68 | `48772e2828d6c7fe0c4bde8b696b1c8a961ed074216af300c9a49a2f963d2828` | 167 |
| `ghostrace-evidence/merged-37ff510/release-native_host_manifest.log` | optimized native_host_manifest suite, 7/7 | `fb3e041037e397da29f3b2baea4d6676c6f86058318c6bddeefe1ab09408a85b` | 854 |

## Privacy, failure, and scope boundaries

- Tests use a temporary `Application Support` root; no real browser directory is touched.
- There is no separate disable state: removing the manifest is how a browser stops launching a host.
