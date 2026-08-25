# Task 0074 evidence: collector feedback-loop and OwnEvent policy

Status: implementation, review, merge, and protected-main verification complete.
PR [#265](https://github.com/AlisinaDevelo/GHOSTRACE/pull/265) was merged to
protected `main` at `46ad1776da9369cd9401f1276371a8e5373826b3`.

The merge-triggered hosted workflows passed for the implementation and test
fix commits: [CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32861276442),
[offline fixture lane](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32861276382),
[Cargo policy](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32861276511),
and [audit](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32861276404).
The receipts below are independent local runs against that exact protected SHA.

## Contract and acceptance mapping

- `InternalPathPolicy` registers journal files, SQLite sidecars, directories,
  exports, backups, and temporary paths with bounded path length and entry
  count. Existing filesystem identity follows a registered directory across a
  relocation; unsafe paths, parent traversal, symlink registration, and
  symlink redirects fail closed.
- The collector checks internal paths before selected-root admission, path
  hashing, or writer submission. Denials are counted and emitted only as the
  path-free `internal_storage_path` policy summary. Other selected-root events
  remain eligible for evidence.
- `OwnEvent` is a serialized observation qualifier and source fact. It does not
  drop an event; operation mapping and contextual evidence remain available.
- A concurrent external write under a registered internal directory is denied
  before persistence. The integration assertion matches the secret path digest
  specifically, allowing unrelated source events while proving the secret was
  not persisted or rendered.

| Criterion | Evidence |
| --- | --- |
| Deny internal paths before persistence across relocation and symlink attempts | `src/fsevents_collector.rs` unit tests `internal_path_policy_tracks_relocation_and_rejects_symlink_redirects`, `internal_path_policy_denies_sidecars_and_descendants_without_rendering_paths`, and `internal_path_policy_rejects_unsafe_inputs_and_is_bounded`; macOS integration `tests/selected_root_collector.rs::internal_storage_writes_are_denied_before_persistence_and_reported_path_free`. |
| Preserve OwnEvent as source evidence | `src/fsevents_collector.rs::tests::own_event_is_evidence_and_never_an_unconditional_drop_rule`; strict JSON round-trip in `tests/fsevents_observation.rs`. |
| Keep concurrent external internal-looking writes denied and diagnostics path-free | The selected-root integration writes a secret from a separate thread, asserts an internal denial counter and summary, matches the secret digest, and rejects both the `FilesystemChanged` payload and raw secret/root text. |

## Target-device verification

The exact protected SHA was checked from a clean detached worktree on macOS
26.6.2 (25G83), Darwin 25.6.0, MacBookPro17,1 / Apple M1 arm64, Rust 1.88.0,
Cargo 1.88.0, and Python 3.9.6. Each receipt is the SHA-256 of the complete
stdout/stderr log for the command.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed; 38 unit tests and all integration targets; one explicitly authorized Keychain lifecycle test ignored | `7e98e5192611cfc8c97ef947fbd09086bf6b94d1536cdfc9bc1d145cb009a081` |
| `cargo +1.88.0 test --locked --all-targets --all-features --release` | passed; 38 unit tests and all integration targets; one explicitly authorized Keychain lifecycle test ignored | `ca3882f5136bf628e6353fe20d7e735b75bf0dcaa686b1bb676b5e27a4595fbd` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `b742b05bf91b9fd6f79b9d70bf8c06a262b52e3aedcf340acdfc87dba2469b61` |
| `cargo +1.88.0 fmt --all -- --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `81b83b19c9d7a219ca637b35b65589610644088ed30c32237c831426e3e5f696` |
| `cargo +1.88.0 rustdoc --locked --all-features -- -D warnings` | passed | `b1ed697157ab5310a44bcaa77ed44ef6824b2ae20568979d6708ec0f58168e87` |
| `scripts/offline-network-test.sh` | passed; sandbox denial canary, privacy fixture, and full offline suite | `4f15aea39e89dc619fc1825023740c74a1bda982b1d73cbc204ad90688e5fb54` |
| selected-root collector debug test | passed; 9/9 | `da78f15e875262d3beeb74a2a13b45ca05bda62ac24bde1d95d27310815b2a26` |
| selected-root collector release test | passed; 9/9 | `903c9f158f98bb9b7839732e18c0c2cbb53c2e637494f4018b3e45f4964f816a` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; 40/40 | `ae455508ef78c94e95e9422adacc8899ad25f359db662e765960be2222792d99` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 47 done, 113 backlog, 488 dependency edges, 0 blocked | `08f9c3dc9cc256fa795353ddc7f09ee4fd2b795ef826824c060793040d4c3335` |

## MVP demonstration

The release binary’s fixture demo completed with exit 0, an 8-event causal
chain, explicit direct/contextual/unknown evidence, and a surfaced coverage
gap. Receipt: `dae831ac26fdd6eb45b5ae77ed3d19e840da2c1ad16664bd4246157c03181ea7`.
The live `capture` command refused with exit 1 and the documented
policy/cursor/Keychain gate message; no live capture was faked. Receipt:
`c5dc8641c56dc406c2ce4591dbf7be9b572399a2a2e51e5b1b4bebd037d366fd`.

## Boundaries and limitations

The internal policy is bounded at 128 registered entries and 4,096 path bytes;
the existing callback, transport, writer, diagnostic, and fault-matrix bounds
remain enforced. The locked Keychain lifecycle test is explicitly ignored
because it requires interactive device authorization; it is not substituted by
hosted CI. Cross-target Linux compilation was not claimed on this device. No
path, display name, account data, credential, file content, or capture key is
retained in the event model or this evidence.
