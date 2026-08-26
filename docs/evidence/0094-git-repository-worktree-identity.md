# Task 0094 evidence: stable Git repository and worktree identity

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#324](https://github.com/AlisinaDevelo/GHOSTRACE/pull/324)
was squash-merged to protected `main` at
`2882c1923ace639b72a9d0582dc7d8545a190246`.

The deliverable is a path-free identity contract for a future explicit Git
adapter. It does not execute Git commands, access remotes, or enable a live
collector.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0094-01 | Identity distinguishes repository object database, worktree, selected root, and source scope. | `GitIdentity` retains an object-database digest, optional worktree digest, opaque `selected_root_id`, `GitSourceScope`, and `GitRepositoryKind`. The object-database digest independently derives the stable event-model `repository_id`, so linked worktrees do not create a second repository. |
| E-0094-02 | Remote URLs, credential helpers, config values, reflog messages, and filesystem paths are excluded or irreversibly minimized. | `GitIdentity` has no fields for those inputs. `GitFilesystemIdentity` keeps device/file values private and only domain-separated SHA-256 digests are serializable. Strict schema/deserialization rejects injected remote, credential-helper, config, reflog, and path fields without echoing their sentinels; a real-directory relocation test finds no source path in JSON or debug output. |
| E-0094-03 | Move, clone, worktree-add, submodule, bare, and repository-reinitialization fixtures define continuity behavior. | [`git-repository-worktree-identity-v1.json`](../../fixtures/git-repository-worktree-identity-v1.json) contains seven deterministic cases: move, clone, linked worktree, submodule, bare repository, repository reinitialization, and source-scope rebinding. The Rust suite verifies `continuous`, `repository_changed`, `worktree_changed`, and `scope_changed` outcomes, including the bare repository's absent worktree digest. |

## Delivery

- Issue: [#98](https://github.com/AlisinaDevelo/GHOSTRACE/issues/98)
- Implementation PR: [#324](https://github.com/AlisinaDevelo/GHOSTRACE/pull/324)
- Implementation commit before squash: `90ae23e1b8ecb96e362ee162b7e847ccdd6273b9`
- Protected-main merge: `2882c1923ace639b72a9d0582dc7d8545a190246`
- Verification date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
merged source revision: 2882c1923ace639b72a9d0582dc7d8545a190246
```

## Merged-main device verification

Every command in this section ran from the exact protected-main SHA above.
Hosted checks are corroboration; the retained device logs are the acceptance
evidence.

### Deterministic, privacy, failure, and recovery lanes

- `CARGO_BUILD_JOBS=1 scripts/reproducibility-test.sh` exited `0`. It checked
  the 21-fixture manifest, schema/golden comparisons, shell lifecycle 7/7,
  shell leakage 6/6, Git identity 6/6, deterministic demo/journal/export/
  retention/integrity/authenticated-state/recovery flows, capture refusal, 46
  Python tests, rustfmt, Clippy with `-D warnings`, and all non-native Rust
  targets. The script skips only its separately authorized native filesystem
  benchmark.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `cargo +1.88.0 test --release --locked --test git_identity -- --nocapture`
  exited `0`; all six identity tests passed in optimized mode.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 test --release --locked --all-targets --all-features
  -- --skip macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt`
  exited `0`; every non-benchmark target passed.

The separate sandboxed `scripts/offline-network-test.sh` lane passed its
network-denial canary, privacy fixture, and complete product suite before the
existing native filesystem benchmark. That unchanged benchmark exceeded its
30-second per-scenario bound after 88.98 seconds and exited `101`; this is an
explicit resource no-go, not a pass or a modified limit.

The Unix relocation test creates only a synthetic directory shape, reads
device/file metadata, moves the directory, and verifies equal identity digests
and `continuous` classification. It does not initialize a real repository,
read file contents, or contact a remote.

## Hosted review and protected merge

PR #324 was pushed from `feature/git-identity-contract` and merged only after
both duplicate push/PR runs were green: audit, Clippy, deny, dependency review,
roadmap, rustfmt, offline fixture/network denial, Linux stable, Linux MSRV, and
macOS stable. No live Git integration or remote/config parser was introduced.

## Retained artifacts

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `/tmp/ghostrace-0094-merged-device-info.txt` | exact device/toolchain capture | `34a0cd9157c9983b368d432f84bacbc955686fa87d271e69a8b02b72dddd7cc1` | 752 |
| `/tmp/ghostrace-0094-merged-repro.log` | merged-main deterministic pipe, exit 0 | `13f643f824def50947724c7dab1e99f83ad06c041a1a0c9edcfc9b90dca18b61` | 41506 |
| `/tmp/ghostrace-0094-merged-release-build.log` | optimized release build, exit 0 | `d51a210ced45ea67de6bcd07ef07231fa0df322004a75ba0a8a46f3a91e44fe1` | 3430 |
| `/tmp/ghostrace-0094-merged-release-identity.log` | optimized identity suite, 6/6, exit 0 | `13945746376e665f2ef3b3f8bbf179cbd2fd4083987dad285b45d65ba6f0384f` | 2566 |
| `/tmp/ghostrace-0094-merged-rustdoc.log` | rustdoc with warnings denied, exit 0 | `8d404389d72e6c0ac6dafe483573490906cec2df3e884e7c5bec67f349aa0d2c` | 630 |
| `/tmp/ghostrace-0094-merged-release-all.log` | optimized all-target/all-feature matrix excluding benchmark, exit 0 | `2fb2bac5da703cd6d06d95dd8bada17af72355d9346be1fb213fb410258ea61d` | 30895 |
| `/tmp/ghostrace-0094-merged-offline.log` | network/privacy/product pass; native benchmark no-go | `3228fa08fee2998cd3ab19c6d423d3720d42d8b33986a6255d5bc7184633a77e` | 12944 |
| `/tmp/ghostrace-0094-merged-python.log` | Python suite, 46/46, exit 0 | `6b7630c5e7f1ba90309b5ef632c193ad7bae28f45fc0eec59a8f2dc139e06fb0` | 145 |
| `fixtures/git-repository-worktree-identity-v1.json` | deterministic fixture registered in manifest | `e5d1d9f2b132ab97e73a502092499e183f077410fc34699aabb62b6abba0d18e` | 5045 |

## Privacy, failure, and scope boundaries

- The fixture and all test inputs are synthetic and offline. The manifest marks
  `user_data_included: false` and `network_required: false`.
- The serializable identity contains only bounded enums, an opaque selected-root
  ID, and tagged digests. Debug output redacts filesystem identity fields, and
  all boundary errors are path-free.
- Continuity is conservative: object-database or repository-kind changes win
  over worktree/scope changes, so a clone or reinitialization cannot be called a
  move. A move remains continuous only when the caller deliberately retains the
  selected-root binding.
- Unix `from_path` rejects symlinks and non-directories and hashes only device
  and inode values. Other platforms return an explicit no-go until an equivalent
  stable filesystem identity adapter exists.
- This task does not parse Git command output, resolve remotes, retain reflogs or
  config, implement hooks, attribute authorship, or establish process causality.
  A future live adapter must prove those boundaries separately before emitting
  Git evidence.
