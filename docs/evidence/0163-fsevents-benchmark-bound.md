# Task 0163 evidence: native FSEvents benchmark within its device bound

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#372](https://github.com/AlisinaDevelo/GHOSTRACE/pull/372)
was squash-merged to protected `main` at `9dad5e04eb490287c6465455f06e9c75f15d10d3`.

The deliverable is the sparse FSEvents cursor fix (#372, fixing #369) and the sandbox-compatible process-inspection check (#376, merged at `ac1d351eaca707dade57118d205db34d123a4fea`).

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0163-01 | A profile identifies where the scenario time goes (event delivery latency, writer commits, journal I/O, or harness waits). | This is a controlled A/B comparison rather than a sampling profiler trace. The same release benchmark on the same device took 41.03 s with roughly one journaled gap per delivered event (git tree 88 events and 77 gaps; event storm 317 and 252), and 18.81 s with 0 gaps once false `cursor_jump` gaps stopped being written. Each gap was its own durable journal commit, so writer commits and journal I/O for false gaps were the dominant cost; the earlier 88-174 s no-go runs were debug builds on a heavily loaded machine. |
| E-0163-02 | The fix keeps the 30-second bound, loss accounting, and durability guarantees unchanged. | The benchmark bound is unchanged. Loss is still recorded from the source's drop and rescan flags (`system_global_id_holes_are_not_recorded_as_lost_events` records a kernel drop as a gap); cursor ordering, regression, and duplicate rules are unchanged, and only the skip rule is limited to contiguous cursors. |
| E-0163-03 | The offline network-denial lane passes end to end on the reference device, including the native benchmark. | `scripts/offline-network-test.sh` exited `0` on `main` at the verified revision: canary, privacy fixture, and the complete product suite including the native benchmark, 338 passed, 0 failed, under a load average of 29. |

## Delivery

- Issue: [#348](https://github.com/AlisinaDevelo/GHOSTRACE/issues/348)
- Implementation PR: [#372](https://github.com/AlisinaDevelo/GHOSTRACE/pull/372)
- Implementation commit before squash: `cad9422103bc8638ee1470fb6987f9630c48e587`
- Protected-main merge: `9dad5e04eb490287c6465455f06e9c75f15d10d3`
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
| `ghostrace-evidence/merged-ce647cb/benchmark-before-5a0bdbb.log` | release benchmark before the fix: 41.03 s, gaps about equal to observed events | `f422328ac893e1491330ea6cf5d0a607816bce2668d56a67e07928bec256976d` | 6996 |
| `ghostrace-evidence/merged-ce647cb/benchmark-after-sparse.log` | release benchmark with the fix: 18.81 s, 0 gaps | `a4a7cccb170ed40baef4bd0d731d3546fdb7e7837f86580d6a62ebfbe7eec8a7` | 6114 |

## Privacy, failure, and scope boundaries

- The before/after receipts come from release builds on the same device; the offline lane is a debug build.
- #376 made the process-inspection exposure test skip where the sandbox forbids `ps`, which the lane reached only once the benchmark passed.
