# Task 0026 evidence: opt-in Git hook install and uninstall

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#362](https://github.com/AlisinaDevelo/GHOSTRACE/pull/362)
was squash-merged to protected `main` at `5a0bdbba25a97009d3bf520b90b83198e65da6ec`.

The deliverable is `plan_chained_install` and `install_chained` in `src/git_hooks.rs`, building on the lifecycle from task 0097.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0026-01 | Installation requires confirmation and is idempotent. | `install_chained` refuses unless the caller passes the digest of the plan it was shown and recomputes it under the lock (`chained_install_requires_the_confirmed_plan_digest`). Idempotency is proven for the plain path (`plan_install_verify_and_uninstall_are_exact_and_idempotent`) and, by a test added with this evidence, for the chained path: a repeated confirmed install reports every hook unchanged and leaves every hook file byte-identical (`repeating_a_confirmed_chained_install_changes_nothing`). |
| E-0026-02 | Existing hooks are preserved and restored. | A user hook is moved to `<hook>.ghostrace-preserved` with its content and mode, still runs first with its original arguments on real commits and checkouts, keeps running while GHOSTRACE is disabled, and is restored exactly by uninstall (`a_chained_user_hook_still_runs_first_with_its_arguments`, `disabling_keeps_the_user_hook_running_and_enable_restores_delegation`, `uninstall_restores_the_original_hook_exactly_and_refuses_if_it_changed`). |
| E-0026-03 | No global Git configuration is changed. | The manager reads only repository `core.hooksPath` scope and writes only the repository hooks directory; `an_existing_preserved_copy_is_never_overwritten_and_global_config_is_untouched` checks a global config file is byte-identical and no hooksPath is written. |
| E-0026-04 | Uninstall behavior is tested. | Uninstall restores preserved hooks, removes shims and the record, refuses on drift of the shim or preserved copy, and a second uninstall is a no-op (chaining and lifecycle suites). |

## Delivery

- Issue: [#30](https://github.com/AlisinaDevelo/GHOSTRACE/issues/30)
- Implementation PR: [#362](https://github.com/AlisinaDevelo/GHOSTRACE/pull/362)
- Implementation commit before squash: `aeb4f234bb674f3477c1329596ecf8b5fa579ea5`
- Protected-main merge: `5a0bdbba25a97009d3bf520b90b83198e65da6ec`
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

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `git_hook_chaining` 5/5; `git_hook_lifecycle` 7/7.

The chained idempotency test added in this change ran with `cargo +1.88.0 test --release --locked --test git_hook_chaining` on `main` at `47645a616ab7fb178d424a24b09e3aeaf00bbc94` plus that test: 6/6 passed.

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
| `ghostrace-evidence/merged-5a0bdbb/release-git_hook_chaining.log` | optimized git_hook_chaining suite, 5/5 | `ee039119eef128091fa6580984e686fba7374977b62b8ba74c75f7f027f9ef98` | 787 |
| `ghostrace-evidence/merged-5a0bdbb/release-git_hook_lifecycle.log` | optimized git_hook_lifecycle suite, 7/7 | `11a28b80e14c8a498788f3a62a9e9bdf968bc8119eceafb3457e568326908354` | 882 |
| `ghostrace-evidence/merged-5a0bdbb/release-git_hook_chaining-idempotency.log` | chaining suite with the idempotency test added in this change, 6/6 | `f70feef0af51c7a0993bb9fb37a84eb8eda17649f10896415f522fd6541d8068` | 851 |

## Privacy, failure, and scope boundaries

- All tests use throwaway repositories with a private HOME and system/global configuration disabled.
- The idempotency test was added in this evidence change and run in optimized mode on main at the revision shown in its log, which contains the merge.
