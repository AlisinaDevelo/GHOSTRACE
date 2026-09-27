# Task 0025 evidence: explicit Git snapshot integration

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#354](https://github.com/AlisinaDevelo/GHOSTRACE/pull/354)
was squash-merged to protected `main` at `24b2af0a8f88bca68d0153b4df29d4a0ca749bb6`.

The deliverable is `GitSnapshotAdapter` in `src/git_adapter.rs`, a policy-gated library adapter that runs hardened read-only Git plumbing for one requested repository. Journal projection and a CLI command are not part of it.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0025-01 | Snapshots contain an opaque repository ID, branch, HEAD, and status counts. | The adapter returns `GitSnapshotMetadata` with a path-free repository ID, algorithm-tagged HEAD and tree IDs, a branch class derived from the ref namespace prefix only, and bounded staged/unstaged/untracked/conflicted counts (`counts_status_classes_and_retains_no_names`, `detached_unborn_bare_and_conflicted_states_are_classified`). |
| E-0025-02 | Diffs, file content, and remote URLs are absent. | Only rev-parse, symbolic-ref, porcelain v2 status, for-each-ref, and config lookups run; no diff or object content is requested, every transport is disabled, and the serialized snapshot contains no names or path separators. |
| E-0025-03 | Hostile branch and path names are parsed and rendered safely. | Status is parsed from NUL-separated records and ref names are never parsed beyond their prefix. `hostile_branch_and_file_names_are_counted_but_never_parsed_as_text` covers command substitution, bidirectional override, newline, tab, emoji, leading dash, and a rename to a name with a newline; `repository_configuration_cannot_execute_programs_during_a_snapshot` proves repository fsmonitor, hook, filter, and pager programs run under plain `git status` but not during a snapshot. |

## Delivery

- Issue: [#29](https://github.com/AlisinaDevelo/GHOSTRACE/issues/29)
- Implementation PR: [#354](https://github.com/AlisinaDevelo/GHOSTRACE/pull/354)
- Implementation commit before squash: `fa1ceea7eedf4be34d298c7c2bf95be12a95ce4a`
- Protected-main merge: `24b2af0a8f88bca68d0153b4df29d4a0ca749bb6`
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
-- --nocapture`, exited `0`: `git_snapshot_adapter` 7/7; `git_snapshot_privacy` 5/5.

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
| `ghostrace-evidence/merged-4b84b42/release-git_snapshot_adapter.log` | optimized git_snapshot_adapter suite, 7/7 | `874eb2b82fb22ea86d22e088482508153b5471f461595325916049bdf1affc09` | 887 |
| `ghostrace-evidence/merged-4b84b42/release-git_snapshot_privacy.log` | optimized git_snapshot_privacy suite, 5/5 | `433a8a76ef7c57d4c203b2c50ddc6264207ae9d93d898fdde9d9d0c88309dfec` | 787 |

## Privacy, failure, and scope boundaries

- Tests use throwaway repositories under a private HOME with system and global configuration disabled.
- Output is capped at 64 MiB and each command at 20 seconds; errors are fixed strings.
