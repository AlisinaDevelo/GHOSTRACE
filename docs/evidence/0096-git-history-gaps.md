# Task 0096 evidence: Git rewrites and unavailable history as gaps

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#345](https://github.com/AlisinaDevelo/GHOSTRACE/pull/345)
was squash-merged to protected `main` at `190b3d142c48a7245bb649f1e64175738762f61f`.

The deliverable is `GitHistoryTransition::classify` in `src/git_history.rs` with the synthetic corpus `fixtures/git-history-transitions-v1.json`. It runs no Git command itself; the adapter's `probe_ancestry` (task 0025) supplies the probe.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0096-01 | The integration distinguishes observed ref movement from inferred commit ancestry. | `ref_movement` is derived only from the two snapshots and `ancestry` only from a definite probe answer, never while replace refs are active. `ancestry_is_never_inferred_without_a_positive_or_negative_probe_answer` checks every corpus case. |
| E-0096-02 | Missing or rewritten history emits a typed gap with the last known and current bounded state. | `GitHistoryGap` carries `history_rewritten`, `object_missing`, `shallow_boundary`, `replaced_objects`, or `ancestry_not_probed` with `last_known` and `current` HEAD, branch class, and shallow/replace/partial states; `every_corpus_case_classifies_as_expected` asserts both sides. |
| E-0096-03 | Fixtures cover rebase, reset, force update, amend, gc, shallow deepen, worktree detach, and object loss. | The registered corpus has 13 cases including all eight; `corpus_is_synthetic_and_covers_every_required_history_change` requires them, and `real_git::real_repository_operations_match_the_contract` reproduces fast-forward, amend, reset, gc after reflog expiry, and detach on throwaway repositories. |

## Delivery

- Issue: [#100](https://github.com/AlisinaDevelo/GHOSTRACE/issues/100)
- Implementation PR: [#345](https://github.com/AlisinaDevelo/GHOSTRACE/pull/345)
- Implementation commit before squash: `eb46fcab191a21e0383a50dcd130bd5190c9ecad`
- Protected-main merge: `190b3d142c48a7245bb649f1e64175738762f61f`
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
-- --nocapture`, exited `0`: `git_history_gaps` 6/6.

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
| `ghostrace-evidence/merged-4b84b42/release-git_history_gaps.log` | optimized git_history_gaps suite, 6/6 | `58a1021b4cfecfac588ad946c95139908e70c9d60a48c3db8ef1507cf266908c` | 2544 |

## Privacy, failure, and scope boundaries

- The corpus is synthetic; the real-repository test uses throwaway temp repositories with a cleared environment and retains only exit status and object IDs.
- Transitions serialize strictly and carry no ref names or paths (`transitions_serialize_strictly_without_ref_names_or_paths`).
