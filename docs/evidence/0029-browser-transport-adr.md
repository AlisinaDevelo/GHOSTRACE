# Task 0029 evidence: browser transport and permissions ADR

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#371](https://github.com/AlisinaDevelo/GHOSTRACE/pull/371)
was squash-merged to protected `main` at `408ddf0cbf0dccaf14b087ae79aaa4c7a0ac6081`.

The deliverable is ADR 0005, `docs/adr/0005-browser-transport-and-permissions.md`, accepted.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0029-01 | The ADR records the Native Messaging and Unix-socket transport choice. | Decision: extension to native host over native messaging, native host to the local service over a user-owned 0700 directory and 0600 Unix socket; the native host never writes the journal. |
| E-0029-02 | Extension allowlist, pairing, message limits, and minimum permissions are documented. | Exactly one extension origin per channel with wildcards refused; user-approved pairing binding the extension key with per-session keys, sequence numbers, and MACs; 64 KiB frames, depth 8, 256 values, four message types, 120 s idle deadline, 200 messages per 10 s; `nativeMessaging` and `webNavigation` only, with excluded permissions listed. |
| E-0029-03 | Private-context policy is documented. | `incognito: "not_allowed"` in the extension manifest plus refusal of any private-context navigation before parsing, counted in a policy-blocked summary. |
| E-0029-04 | Localhost HTTP is explicitly rejected. | The decision rejects any TCP listener, local HTTP server, WebSocket, or `externally_connectable` origin, with the reasons, and the alternatives section records the rejected localhost server. |

## Delivery

- Issue: [#33](https://github.com/AlisinaDevelo/GHOSTRACE/issues/33)
- Implementation PR: [#371](https://github.com/AlisinaDevelo/GHOSTRACE/pull/371)
- Implementation commit before squash: `4ad4cd34abe2d0115e0ab712bf22bb16854ecf22`
- Protected-main merge: `408ddf0cbf0dccaf14b087ae79aaa4c7a0ac6081`
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
| `ghostrace-evidence/merged-ce647cb/adr-0005-corpus.log` | ADR/corpus consistency suite on main at 408ddf0, 4/4 | `c0cc1b20cc48f8fd773cb7176cba8b1800ac2b575b47009da469803aa76f431c` | 782 |

## Privacy, failure, and scope boundaries

- The ADR merged after the main verification revision; its consistency test ran separately on `main` at `408ddf0cbf0dccaf14b087ae79aaa4c7a0ac6081`, which contains the merge.
- The ADR records decisions; the codec, service, installer, and pairing that implement them are tasks 0102-0104 and 0111.
