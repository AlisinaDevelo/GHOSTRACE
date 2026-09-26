# Task 0095 evidence: minimized Git refs, object IDs, and snapshot fields

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#326](https://github.com/AlisinaDevelo/GHOSTRACE/pull/326)
was squash-merged to protected `main` at
`743fc17ea9ef9c410e6869cd2cd9ccb73223a30d`.

The deliverable is the metadata-only snapshot contract in `src/git_snapshot.rs`.
It does not run Git, read objects, or enable a live Git adapter.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0095-01 | Object IDs use validated algorithm-aware formats and never cause object content reads by default. | `GitObjectIdRef` accepts only `sha1:` + 40 or `sha256:` + 64 lowercase hex and exposes no object-reading operation; `GIT_OBJECT_READ_POLICY` is `metadata_only`. `object_ids_are_explicitly_algorithm_aware_and_content_free` and `object_format_and_digest_changes_fail_closed_without_echoing_input` cover malformed IDs, algorithm/format mismatch, and digest drift. |
| E-0095-02 | Ref names, commit messages, authors, remotes, diffs, patches, filenames, and untracked content are excluded from the baseline. | `GitSnapshotMetadata` retains only branch class, worktree state, operation class, and bounded status counts; there is no field for any excluded value. Strict schema and deserialization reject injected fields without echoing them (`snapshot_captures_bounded_status_and_operation_facts_without_names`, `checked_in_contract_is_strict_bounded_and_deterministic`). |
| E-0095-03 | Snapshots expose source limitations for partial clones, replace refs, shallow history, submodules, and alternate object databases. | `GitSourceLimitations` is required and has explicit `unknown` states for each condition; submodule snapshots must declare submodules and bare repositories have no worktree (`source_limitations_are_required_and_bare_repositories_have_no_worktree`). |

## Delivery

- Issue: [#99](https://github.com/AlisinaDevelo/GHOSTRACE/issues/99)
- Implementation PR: [#326](https://github.com/AlisinaDevelo/GHOSTRACE/pull/326)
- Implementation commit before squash: `c9f180dafe13dcba46accd9d95a58defe02e5cdb`
- Protected-main merge: `743fc17ea9ef9c410e6869cd2cd9ccb73223a30d`
- Verification date: 2026-09-27 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 905c11e6895a63c6a5b38a2973e64686b9321b4b
```

## Merged-main device verification

Every command ran from protected `main` at
`905c11e6895a63c6a5b38a2973e64686b9321b4b`, which contains this task's merge.
Hosted checks are corroboration; the retained device logs are the acceptance
evidence.

- `bash scripts/reproducibility-test.sh` exited `0`: pinned inputs, rustfmt,
  schema, Parquet profile, shell metadata/lifecycle/leakage, Git identity, Git
  snapshot privacy, deterministic demo/journal/export/retention/integrity/
  authenticated-state/recovery flows, capture refusal, the Python evidence, and
  the Rust evidence all passed.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 274 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 51 tests.

Not run in this verification: the sandboxed `scripts/offline-network-test.sh`
lane and the native filesystem benchmark. The benchmark's standing resource
no-go on this device is tracked separately in task 0163
([#348](https://github.com/AlisinaDevelo/GHOSTRACE/issues/348)); it is not
claimed as a pass here.

The focused optimized run `cargo +1.88.0 test --release --locked --test
git_snapshot_privacy -- --nocapture` exited `0` with 5/5 tests.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-905c11e/device-info.txt` | exact device/toolchain capture | `adb8595f0d125f6f101683a3611f9283f2c608861380a91203c3f33d3aeced09` | 230 |
| `ghostrace-evidence/merged-905c11e/repro.log` | reproducibility lane, exit 0 | `d6ac200dbeb799def517ed268ab0889130fb7318fb389bc48a975cb1ee2c1515` | 41622 |
| `ghostrace-evidence/merged-905c11e/clippy.log` | Clippy with warnings denied, exit 0 | `ddcb06ce53600b5701e9bb1c7de36d92136d672683aa24d86c6993e714b4e4ec` | 114 |
| `ghostrace-evidence/merged-905c11e/all-targets.log` | all targets except native benchmark, 274 passed | `0e3d39a527e6d18659fc2343e6dbf827f5c5e94687786ab82adc3b226db9528e` | 34606 |
| `ghostrace-evidence/merged-905c11e/rustdoc.log` | rustdoc with warnings denied, exit 0 | `2409b54ac218a6e0c805348e3f53dc92ce786a1c947fb00d5816c5d24c049e7d` | 602 |
| `ghostrace-evidence/merged-905c11e/release-build.log` | optimized release build, exit 0 | `09188ec28550ccfa239af0cd174aa6597d532e3d6b61ffac4c2449c1cdd7943d` | 3438 |
| `ghostrace-evidence/merged-905c11e/release-git_snapshot_privacy.log` | optimized Git snapshot suite, 5/5 | `d323418bcbf9ee4cf4d5f1da454b2cfd6d88856c5f2162490a61374536699b13` | 2526 |
| `ghostrace-evidence/merged-905c11e/python.log` | Python suite, 51/51 | `1ae2f4741ce4a6f4dac9b141f2500cfae4d87e6edee92439286e5f49716c50d4` | 150 |
| `fixtures/git-snapshot-metadata-v1.golden.json` | registered golden snapshot | `8b36de1231a08e82ebde44e7052de56d65cde6106169969173b6713b5b890289` | 781 |

## Privacy, failure, and scope boundaries

- All inputs are synthetic; the golden snapshot is registered in the fixture
  manifest with no user data and no network requirement.
- Boundary errors are fixed strings and never include untrusted Git text.
- Unknown source conditions stay `unknown`; they are never promoted to complete
  history. History rewrites and unavailable objects are handled by task 0096.
- No Git command execution, consent wiring, or event projection is part of this
  task.
