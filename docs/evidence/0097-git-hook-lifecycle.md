# Task 0097 evidence: verifiable and reversible Git hook installation

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#355](https://github.com/AlisinaDevelo/GHOSTRACE/pull/355)
was squash-merged to protected `main` at `4b84b424f76b441b1b328330301b22961634390c`.

The deliverable is `GitHookManager` in `src/git_hooks.rs`, a library manager for repository-local shims with a digest record.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0097-01 | Plan, install, verify, upgrade, disable, and uninstall operations are idempotent and show exact affected files. | Every operation returns each affected file and action; repeating install, disable, or uninstall is a no-op (`plan_install_verify_and_uninstall_are_exact_and_idempotent`, `disable_and_enable_are_reversible_and_stop_delegation`, `upgrade_rewrites_only_intact_shims_and_covers_linked_worktrees`). |
| E-0097-02 | Existing hooks, core.hooksPath, worktrees, symlinks, ownership, modes, and concurrent edits are preserved or cause refusal. | A foreign hook keeps its content and mode and nothing is written (`existing_user_hooks_are_preserved_by_refusing_before_any_write`); `core.hooksPath`, symlinked hooks directories and hooks are refused (`a_configured_hooks_path_or_symlinked_hooks_are_refused`); linked worktrees resolve to the common hooks; foreign ownership is refused; a held lock and exclusive-create races refuse (`concurrent_operations_and_invalid_delegates_are_refused`). |
| E-0097-03 | A signed or checksummed shim delegates safely and uninstall removes only artifacts whose identity still matches. | Each shim's SHA-256 and mode are recorded; the shim runs a validated absolute delegate with stdin closed and no hook arguments; `drifted_shims_are_reported_and_never_removed` shows edited shims survive uninstall and disable, and uninstall re-checks each digest immediately before removal. |

## Delivery

- Issue: [#101](https://github.com/AlisinaDevelo/GHOSTRACE/issues/101)
- Implementation PR: [#355](https://github.com/AlisinaDevelo/GHOSTRACE/pull/355)
- Implementation commit before squash: `2ca44a685bb4f9671924984b263ffe0495c73106`
- Protected-main merge: `4b84b424f76b441b1b328330301b22961634390c`
- Verification date: 2026-09-27 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 4b84b424f76b441b1b328330301b22961634390c
```

## Merged-main device verification

Every command ran from protected `main` at `4b84b424f76b441b1b328330301b22961634390c`, which contains this task's
merge. Hosted checks are corroboration; the retained device logs are the
acceptance evidence.

- `bash scripts/reproducibility-test.sh` exited `0` ("all checks passed").
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 300 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 51 tests.

Not run in this verification: the sandboxed `scripts/offline-network-test.sh`
lane and the native filesystem benchmark, whose resource no-go on this device is
tracked in task 0163 ([#348](https://github.com/AlisinaDevelo/GHOSTRACE/issues/348)).

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `git_hook_lifecycle` 7/7.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-4b84b42/device-info.txt` | exact device/toolchain capture | `d721fd33fefaa221a0550111da4bcecec17d88b41782f1584af2ef365d943d79` | 230 |
| `ghostrace-evidence/merged-4b84b42/repro.log` | reproducibility lane, exit 0 | `3691ebb1b5bdbc66629b7e2da733389532bb235edbb9c83035b1cf67ba455fce` | 44792 |
| `ghostrace-evidence/merged-4b84b42/clippy.log` | Clippy with warnings denied, exit 0 | `1c8d1f786821918fdcf1365da4e2279f678760ad63fee273a38ff82977f19ffb` | 60 |
| `ghostrace-evidence/merged-4b84b42/all-targets.log` | all targets except native benchmark, 300 passed | `c432cda2ec58d0eca3b2eba9deb82daaad9cab47ea992f61992c1b58ce31e8ca` | 37430 |
| `ghostrace-evidence/merged-4b84b42/rustdoc.log` | rustdoc with warnings denied, exit 0 | `d2f00845cc6eeea27da7b254f9187086de90f3b9503cd78eec5e89027bf79ee4` | 656 |
| `ghostrace-evidence/merged-4b84b42/release-build.log` | optimized release build, exit 0 | `a9c9a28f560875c357dbb15ee4e7225b97c5688a3129fe05f564afe4d9249b74` | 3438 |
| `ghostrace-evidence/merged-4b84b42/python.log` | Python suite, 51/51 | `568f4fec8b5c5d30958b68d7a2c4f7ba9726fd9e513ac75b14998c2096d7c852` | 150 |
| `ghostrace-evidence/merged-4b84b42/release-git_hook_lifecycle.log` | optimized git_hook_lifecycle suite, 7/7 | `c9170d33febe09bf4a372a8135145993c848a7c9aaa33aae161c26704502be51` | 882 |

## Privacy, failure, and scope boundaries

- A real commit in a throwaway repository runs the delegate; a disabled shim does not.
- No global Git configuration is read or written. Chaining existing user hooks instead of refusing is task 0026.
