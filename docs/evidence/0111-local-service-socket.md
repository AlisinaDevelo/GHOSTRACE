# Task 0111 evidence: authenticated local Unix-socket protocol

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#373](https://github.com/AlisinaDevelo/GHOSTRACE/pull/373)
was squash-merged to protected `main` at `eee21a5d5f19a5f9e1a3b6b0f33f3ba1f79dccef`.

The deliverable is `LocalService` in `src/local_service.rs`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0111-01 | Socket directory and file ownership and mode are verified without following links, and no TCP listener is created. | The directory must be a real user-owned directory with no group or other access (created 0700), symlinks are never followed, the socket is 0600, and only a stale user-owned socket is replaced (`socket_is_private_and_a_granted_request_is_answered`, `unsafe_directories_and_socket_paths_are_refused_without_following_links`); `the_module_opens_no_network_listener` fails if a network listener appears. |
| E-0111-02 | Peer credentials, service instance, protocol version, request size, deadlines, and replay semantics are validated before dispatch. | The peer must be the same effective user (`getpeereid` on macOS, `SO_PEERCRED` on Linux); requests are at most 64 KiB of strict JSON, version 1, addressed to the per-start instance ID, with a 1 ms-30 s deadline and an unseen request ID (`version_instance_deadline_and_replay_are_checked_before_dispatch`, `oversized_malformed_and_unknown_field_requests_are_refused`, `a_mismatched_peer_is_rejected_before_the_request_is_parsed`). |
| E-0111-03 | Read, export, policy, lifecycle, and administrative capabilities are separate and denied by default. | Capabilities are granted explicitly at bind; each ungranted one and an empty grant set are refused (`capabilities_are_separate_and_denied_by_default`). |

## Delivery

- Issue: [#115](https://github.com/AlisinaDevelo/GHOSTRACE/issues/115)
- Implementation PR: [#373](https://github.com/AlisinaDevelo/GHOSTRACE/pull/373)
- Implementation commit before squash: `8f04f27f2e603c01d81eef50b797c4b2bebbaa3d`
- Protected-main merge: `eee21a5d5f19a5f9e1a3b6b0f33f3ba1f79dccef`
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
-- --nocapture`, exited `0`: `local_service` 7/7.

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
| `ghostrace-evidence/merged-37ff510/release-local_service.log` | optimized local_service suite, 7/7 | `67774337af40715eaa0b78272421f61fbe9b1c27ae4d51b49458a5b9fe6ad004` | 869 |

## Privacy, failure, and scope boundaries

- The service has no methods yet; this task is its admission layer.
- Linux peer credentials are covered by hosted CI; the device run is macOS.
