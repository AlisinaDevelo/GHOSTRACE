# Task 0073 evidence: FSEvents delivery observation contract

Status: implementation and protected-main verification complete. PR [#262](https://github.com/AlisinaDevelo/GHOSTRACE/pull/262)
was reviewed and merged to protected `main` at
`47ed8164d55462befe8e0b8194245dd2b4bfa516`.

The merge-triggered hosted workflows for that exact protected commit all
passed: [CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32855922256),
[Rust advisories](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32855922084),
[Cargo policy](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32855922017),
and [offline fixture lane](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32855922007).

## Contract and acceptance mapping

The selected-root filesystem payload now has explicit, path-free delivery
qualifiers:

- `observation: source_coalesced` records that a stream without per-file
  delivery may have combined source changes.
- `observation: repeated_modification` records a distinct source delivery for a
  path digest already admitted as created or modified.
- Exact transport re-deliveries are not persisted. The deterministic key is
  `(source event ID, raw flag word, path digest)`, retained for at most 1,024
  keys and a 4,096 event-ID horizon. Suppressed deliveries are exposed only as
  the path-free `CollectorStatus.transport_duplicates` counter. A different
  source ID, flag word, or path digest is distinct evidence.
- `rename_pairing` is `unknown` for the current one-digest callback boundary;
  `contextual` is reserved for a future bounded relationship. There is no
  inferred wire value, and a rename qualifier is valid only with a `renamed`
  operation. Summaries state the limitation without a plaintext path.

| Criterion | Evidence |
| --- | --- |
| Distinguish coalescing, transport duplication, repeated modification, and rename pairing | `tests/fsevents_observation.rs::observation_contract_is_path_free_and_strict` rejects transport-duplicate and inferred-rename wire values; `src/fsevents_collector.rs` classifies coalescing/repetition and records the transport counter; selected-root integration asserts contextual unknown rename evidence. |
| Deterministic bounded deduplication never erases distinct source evidence | `src/fsevents_collector.rs::tests::transport_dedup_is_exact_bounded_and_time_scoped` proves exact-key behavior, distinct event IDs/flags, horizon expiry, and the 1,024-key bound. |
| Rename explanations state unknown or contextual pairing | `tests/selected_root_collector.rs::selected_root_collector_captures_controlled_file_lifecycle_without_content` and `tests/vertical_slice.rs::rename_summary_states_unknown_pairing_without_a_path` assert contextual evidence, `unknown` pairing, and path-free summaries; `tests/fsevents_observation.rs::rename_pairing_requires_a_renamed_operation` covers the negative cross-field case. |

## Target-device verification

The exact protected commit above was checked from a clean worktree on macOS
26.6.2 (25G83), MacBookPro17,1 / Apple M1 arm64, Rust 1.88.0, Cargo 1.88.0,
and Python 3.9.6. Receipt hashes are SHA-256 of complete stdout/stderr logs
from each command.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed; 34 unit tests and all integration targets; one explicitly authorized Keychain lifecycle test ignored | `f24b445c8bc485af64f2f2b2508c1a8965e46bfbc10128e8d8e34462a9761d77` |
| `cargo +1.88.0 test --locked --release --all-targets --all-features` | passed; one explicitly authorized Keychain lifecycle test ignored | `650b6630a58e827bc9051a414964da3fcda6a3fd9a190e2b2432153061929e7c` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `ea0e74f5d1ae2f42bcbaa7a097dbbd201d0f25627785484844f1362d08f93cf6` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `e8f0a959843461f09250ca46d6f0e0ad318d437c82a85a20371fba34455a655b` |
| `cargo +1.88.0 fmt --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps --all-features` | passed | `af48a7148ac41c7a869d95bebdd7ed328ec6e40c9f62f19dfee78790d47d5a1e` |
| `./scripts/offline-network-test.sh` | passed; network-denial canary and full offline suite | `db5f756878f38b362bc0c071d2473bee55bf2ee2541ea5b752297150f386e844` |
| `./scripts/reproducibility-test.sh` | passed; pinned inputs, deterministic demo/export, Python 40/40, and Rust evidence | `f7bd521ffb3e71cd6606e7844ab3a8f153080f9f483aa585e61c5942880fb747` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; 40/40 | `b428019a31de87e3f073a5de29ec8fbff99df8a5f225c2b2b3677afc27642f08` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 47 done, 113 backlog, 488 dependency edges, 0 blocked | `08f9c3dc9cc256fa795353ddc7f09ee4fd2b795ef826824c060793040d4c3335` |
| selected-root collector debug test | passed; 8/8 | `7f4c316dd0eb4946727f6557600591d4674cf47a3861574e5feeec2fb237a504` |
| selected-root collector release test, run 1 | passed; 8/8 | `3ba1c4e8a103fb765c103fde06738ab6bc59d6ade5e5837ac982169446373685` |
| selected-root collector release test, run 2 | passed; 8/8 | `82df4e75be5418e12d78d0f937a883ae1089dfea0d751b86ed00118aba7bb9c2` |
| selected-root collector release test, run 3 | passed; 8/8 | `d55a79b68ee61a7b5d45e44b42b39aeaaff3540ac49e9b9d47c330fe0ae31228` |

## Explicit limitations

The local `x86_64-unknown-linux-gnu` cross-target attempt was made against
this exact SHA and stopped before compilation because this Mac does not have
`x86_64-linux-gnu-gcc`; its complete receipt is
`1378af5a488414c40c814cac03779ddbd14f3a0b6752aed3b237d2e11a025e3b`. Hosted
Linux stable/MSRV runs passed in the linked CI workflow. The locked-session
Keychain lifecycle test remains ignored because it requires explicit device
Keychain authorization; it is not replaced by CI. No path, display name,
account data, credential, or capture key is retained in this evidence.

## Live GitHub reconciliation

After the evidence receipt was merged and issue #77 was updated, independent
authenticated reads of `AlisinaDevelo/GHOSTRACE` reported exactly 160 issues
(113 open, 47 closed), 12 milestones (`M0` through `M11`), and 31 managed
labels (45 labels total). Issue #77 is closed with reason `completed` and has
`status:done`; it has no `status:backlog` label.

The versioned task tree digest is
`711589cbeeca219b3af72702e2ab94244b919fddad5cb3f12941cda0971587e9`.
The inspected local Forge parity receipt has no operations and SHA-256
`c8500168ea0d4ac4c6a79ca76e7392603bd67919eb92dc48a9c1bd25f1e7be86`.
The authenticated metadata planner was run twice after the issue update. Both
plans returned zero operations, zero blockers, the same plan digest
`885b2639de853f32ea0a20202deb81907e4c3ecf96bffff1eb5803d91486f8d0`, and
byte-identical JSON receipts with SHA-256
`0ae0c3718c972bedaf324576ce59861c438c5685c6a0bd3a3c38940715534264`. No
metadata apply was needed. The raw independent live-read receipts are also
hash-pinned: issue list `b5f2dfefbcc3d7716a3c0d2328626dbae51ab336909bee38ba66b9b12139a659`,
labeled issue list `aaa270cc1afd70e1ed04dcb7fc84a27096e1b19ee78ce90d964930df4af1cc17`,
label list `509495234aea420ddc5c6c27e9c6696ee7797f1ddb3bb6be8bc278920cba1e85`,
and milestone list `b972db09e86a4da38ce904fd559c2d83f51cde2a41b40c66cdf1659f6f9a62dd`.
