# Task 0075 evidence: storm and lifecycle corpus

Status: implementation, review, merge, and protected-main verification complete.
Pull request [#268](https://github.com/AlisinaDevelo/GHOSTRACE/pull/268) was
merged to protected `main` at
`e6bd66389b903354d781763391ebcb04441dabf3`. The task ledger is now `done` and
the public issue is closed only after this evidence is merged.

## Contract and acceptance mapping

The public contract is
[`fixtures/fsevents-lifecycle-corpus-v1.json`](../../fixtures/fsevents-lifecycle-corpus-v1.json),
validated by `scripts/fsevents-lifecycle-corpus.py` and included in the fixture
manifest. It has nine deterministic rows:

| Row | Device status | Ground truth and direct observation contract | Gap/recovery gate |
| --- | --- | --- | --- |
| bulk checkout | native-safe | 16 creates; direct `created`; same-directory callback coalescing permitted | none |
| package install | native-safe | temp create/modify, atomic rename, manifest create; direct create/modify/rename | temp modification may coalesce |
| rename storm | native-safe | 8 creates plus 8 renames; direct create/rename | burst coalescing and unknown pairing permitted |
| directory deletion | native-safe | directory/tree creates followed by recursive deletion; direct create/delete | parent-only recursive delete permitted |
| sleep/wake | guarded no-go | sleep and wake operation log; no direct filesystem claim | requires `fsevents_history_incomplete` and reconciliation |
| logout | guarded no-go | logout operation log; no direct filesystem claim | requires `fsevents_history_incomplete` and reconciliation |
| volume detach | guarded no-go | volume-detach operation log; no direct filesystem claim | requires `fsevents_root_changed` and reconciliation |
| process kill | native-safe | child marker create plus process termination | termination is not process attribution |
| restart | native-safe | explicit collector stop/start plus post-restart marker | resume must be observed before a live claim |

The validator checks operation sequence numbers, bounded identifiers, direct
observation lists, permitted coalescing, required gaps, recovery expectations,
resource limits, synthetic-only privacy, and the absence of path/content fields.
It then replays the pinned projection 32 times. The final merged-main replay
receipt is `d8754ba160e3d9c4fff96e4b47f64ee0ea919948890b5df5cd5f69a1b872eb70`.
The aggregate fixture-only distribution was:

```json
{"duplicate_rate":0.01290761,"omission_rate":0.00747283,"ordering_inversion_rate":0.02777778,"recovery_success_rate":1.0,"resource_peak_events":17}
```

This is deterministic fixture validation, not native-device evidence.

## Target-device verification

The protected SHA was checked from a fresh detached worktree on the target
device. The complete device/toolchain receipt is
`84d02c873fc253f0849bca55ec087446f1b6c44031a003a9a5abc87fb19add6c`.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1) |
| Source | protected `main` `e6bd66389b903354d781763391ebcb04441dabf3` |
| Rust/Cargo | rustc/cargo 1.88.0, host `aarch64-apple-darwin` |
| Python | 3.9.6 |

The complete local logs are retained under `/tmp/ghostrace-0075-main-*`; each
hash below covers the command's combined stdout and stderr.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed; all targets, 38 unit tests, native FSEvents rows, one pre-existing authorized Keychain test ignored | `f480292350b8c07c73cbcf4cfe794385d8135719563964463ed927739ca1d8ae` |
| `cargo +1.88.0 test --locked --all-targets --all-features --release` | passed; all targets, one pre-existing authorized Keychain test ignored | `09e69ed9144ebaa7dede07c13bac7d9610dc2ebd6ff846fdb6fcf3d51fd492f9` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `cf677b7e847b4cc3768ea1e637a3d360a4a3e655bd022448645167d9a09b256f` |
| `cargo +1.88.0 fmt --all -- --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `4d103529f1053702f237db4cd78c3479b74a23135b5da7c49a82859548157669` |
| `cargo +1.88.0 rustdoc --locked --all-features -- -D warnings` | passed | `0fad1c73d12175f5acd6a4e6e6da79034bd13035e07f18b069df2810c0a2f4f1` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; all Python tests | `0ea03ebfae65c1fe683bd25f22c7b7ea55e36471e2e33f954cc64b0b770c5ccb` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 49 done, 111 backlog, 488 dependency edges, 0 blocked | `f87838816f072e699d69c6167bd1ffc39271609937ff7cf7033966403e465086` |
| `scripts/offline-network-test.sh` | passed; enforced network-denial canary, privacy lane, and offline all-target suite | `8ac94f900a6a268d55c6ec36cbaf3aedd029c465b4e940c13183647d20d76b44` |
| `scripts/reproducibility-test.sh` | passed; pinned inputs, schema, deterministic demo, durable reopen/export, Python, Clippy, and Rust | `6fa966fce159dfa392e6f216ca1b5df352e9080feca6c842ce703173b84825e5` |
| `scripts/fsevents-sanitizer.sh` | passed; nightly AddressSanitizer native FSEvents lifecycle target | `6a8358e4fa51f6284726e0e07fb3c7c2abd8b98daddd21554c28f9ec1cc87c44` |

### Native lifecycle receipt

The new macOS integration ran three rounds of each device-safe row on a private
mode-0700 temporary selected root. It created a bulk checkout corpus, performed
a package-like temp-write/rename, generated a rename storm, recursively deleted
a directory tree, killed a child process after a marker write, and stopped and
restarted the collector before writing a post-restart marker. The test never
prints paths or file contents.

| Build | Result | Receipt |
| --- | --- | --- |
| debug native corpus test | passed; 3/3 restart recoveries, 171 observations, 0 source-ID duplicates, 0 ordering inversions, 0 drops, 0 transport duplicates | `5de450b8689d2911af0832870f32d8db2a060c4ea0f09d359a118c818ab599a3` |
| release native corpus test | passed; 3/3 restart recoveries, 172 observations, 0 source-ID duplicates, 0 ordering inversions, 0 drops, 0 transport duplicates | `bb449606d0293269107730aa95dfd7a0c84bbbc8cba7cb597efb95bd7c0ad6d7` |

FSEvents is lossy: the debug and release counts differ by one because callback
coalescing is allowed by the fixture contract. The receipt records the observed
operation counts and bounded counters rather than converting a shortfall into a
false one-to-one claim.

## Public checks and merge

The final PR head `bcd46a026ea048db5e6af1a5ec554cbac234e94a` passed all required
repository checks after a portability correction that moved macOS-only imports
inside the target-gated module. The protected-main merge checks all passed:

- [CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32865930113)
- [offline fixture lane](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32865930124)
- [Cargo policy](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32865930093)
- [Rust advisories](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32865930111)

Hosted checks corroborate the repository build; they are not substituted for the
target-device receipts above.

## MVP demonstration

The merged-main release binary's fixture demo completed with exit 0, an eight-event
causal chain, direct/contextual/unknown evidence, and an explicit coverage gap.
Receipt: `34e13dab820cfb719c862bffde4485ae07d34b1b2ba30c3be7d6e7e8a25b81cf`.
The live `capture` command refused with exit 1 and the documented policy/cursor/
Keychain gate message; no live capture was faked. Receipt:
`586d32f0af0c6a468c249db9e94b2a73aea16b5084ec68795f55b8388f968eaa`.

## Boundaries and limitations

- Sleep/wake, logout, and volume detach were not triggered. They would suspend or
  terminate the active session or risk user data, so they remain explicit no-go
  rows with required gaps. No hosted CI or synthetic replay is counted as native
  evidence for them.
- The package-install row is a safe atomic-write/rename proxy, not a real package
  manager transaction. The process-kill row observes filesystem metadata only and
  makes no actor-attribution claim.
- FSEvents can omit, coalesce, reorder, or delay events. The contract therefore
  reports shortfalls, duplicate counters, ordering, and recovery rather than
  asserting exact operation reconstruction.
- No path, filename, account name, credential, display title, file content, or
  network payload is retained by the corpus or receipt.
