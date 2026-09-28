# Task 0115 evidence: release entitlements and permission drift

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#353](https://github.com/AlisinaDevelo/GHOSTRACE/pull/353)
was squash-merged to protected `main` at `2a18133eb7f2bdc0fe6b2482f4e56d7b9a7e8ba0`.

The deliverable is `planning/permission-manifest.json`, `scripts/permissions.py`, and the macOS `permissions` CI job.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0115-01 | CI extracts and compares signed entitlement and bundle metadata against the reviewed manifest. | The `permissions` CI job builds the release binary and runs `scripts/permissions.py check --binary`, which reads entitlements with `codesign` and load commands with `otool -L`. On this device the merged release binary matched its reviewed entry (ad-hoc linker signature, no entitlements, five system libraries). |
| E-0115-02 | New or broadened permissions fail until privacy, threat, test, and migration evidence is approved. | `approved_permission_digest` covers every permission-relevant field and names the privacy, threat-model, test, and migration evidence; `test_new_or_broadened_permission_fails_until_review_digest_is_updated` mutates entitlements, libraries, privacy APIs, filesystem rights, sandbox state, and helpers. |
| E-0115-03 | Release evidence proves no debug, get-task-allow, disable-library-validation, unexpected network, or overbroad sandbox exception is present. | Forbidden entitlements (get-task-allow, disable-library-validation, allow-jit, network, temporary exceptions) fail even in an approved manifest, network linkage and network capability fail, and `test_resigned_binary_with_debug_entitlement_is_rejected` proves a real binary re-signed with get-task-allow is rejected. The checked release binary carried none. |

## Delivery

- Issue: [#119](https://github.com/AlisinaDevelo/GHOSTRACE/issues/119)
- Implementation PR: [#353](https://github.com/AlisinaDevelo/GHOSTRACE/pull/353)
- Implementation commit before squash: `0f519c89b46b0cbc79b2651796e216161fb6d4bb`
- Protected-main merge: `2a18133eb7f2bdc0fe6b2482f4e56d7b9a7e8ba0`
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

`python3 scripts/permissions.py check --binary target/release/ghostrace` exited `0` against the merged release build, and `python3 -m unittest discover -s tests -p 'test_permissions.py' -v` exited `0` with 10 tests.

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
| `ghostrace-evidence/merged-5a0bdbb/permissions-binary.log` | binary check of target/release/ghostrace, ok | `87a1387d25e973cd01231603f6c0766ecb011cc3d13527b6809b2f66a64924c1` | 155 |
| `ghostrace-evidence/merged-5a0bdbb/permissions-unit.log` | permission contract suite, 10/10 | `777eea570b7562ad186836468ba4904f0536df7fc02ff4c1d1457708d1055e17` | 1030 |

## Privacy, failure, and scope boundaries

- The job is not yet a required status check; adding it to branch protection is a repository setting.
- Developer ID signing, bundles, and notarization (tasks 0116–0118) must add their entries before they ship.
