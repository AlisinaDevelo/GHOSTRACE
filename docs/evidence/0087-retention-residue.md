# Task 0087: deletion residue limits

Status: complete on protected `main`.

This receipt documents the read-only residue boundary shipped for issue #91. It
does not claim that GHOSTRACE can erase bytes from SQLite free pages, WAL/SHM,
virtual-table shadow storage, backups, filesystem snapshots, SSD wear-levelling
media, or privileged recovery media.

## Delivery

- Issue: [#91](https://github.com/AlisinaDevelo/GHOSTRACE/issues/91)
- Implementation PR: [#304](https://github.com/AlisinaDevelo/GHOSTRACE/pull/304)
- Implementation commit before squash: `02d3db482c75670bafcb3bbea749d52452a9d812`
- Protected-main merge: `da5c34e018430c229c000f14feaa108cb425dd20`
- Reproduction date: 2026-08-26 UTC

The CLI now exposes a deterministic, path-free `residue-report` command. It
reports database, WAL, SHM, rollback-journal, temporary, backup, FTS-shadow,
and archive-shadow classes, plus the SQLite `secure_delete` setting. Its mode
contract keeps logical deletion, compaction/VACUUM, cryptographic erasure, and
external-copy responsibility distinct.

## Acceptance evidence

| Evidence | Acceptance criterion | Result |
|---|---|---|
| E-0087-01 | Guarantees, costs, unsupported media, `secure_delete`, and `VACUUM` behavior are explicit. | Four serialized mode descriptions, privacy documentation, and evaluation documentation are present and validated in the merged reproducibility lane. |
| E-0087-02 | Sentinel checks cover database, WAL, SHM, temporary, FTS/archive shadow structures, and backups where meaningful. | The merged focused lane passed the database/sidecar/FTS/archive/backup sentinel matrix; post-delete bytes are recorded observationally rather than treated as universal erasure. |
| E-0087-03 | The available CLI distinguishes the four responsibilities, with a contract for a future UI. | `residue-report` emits all four stable mode labels and their limits; the current fixture baseline has no shipped Tauri UI, so no UI behavior is claimed beyond the documented contract. |

## Device and toolchain

The merged-main receipt was produced on the target development device:

```text
Darwin 25.6.0 / macOS 26.6.2 (25G83)
MacBookPro17,1-class Apple Silicon / arm64
rustc 1.88.0 (LLVM 20.1.5), host aarch64-apple-darwin
Python 3.9.6
```

## Merged-main verification

Every lane below ran from `da5c34e018430c229c000f14feaa108cb425dd20` in a
fresh evidence worktree. The digest is SHA-256 of the retained local log.

| Receipt | Command/result | Log SHA-256 |
|---|---|---|
| E-0087-04 | `scripts/reproducibility-test.sh` — all checks passed; locked Clippy and all debug targets passed; 46 Python tests passed. | `721ce0384858c5af22849ab42f3089dfff50697f1669bf7da9119d7bdedde422` |
| E-0087-05 | `scripts/reproducibility-test.sh --offline-network` — all checks passed with network denied. | `cee600e7707cab589cfacb0478f33fa08308d4ea1eed2e4303ea09d354321749` |
| E-0087-06 | `scripts/fsevents-sanitizer.sh` — native lifecycle passed; three documented `*_fetchInitializingClassList*` suppressions. | `0bb0fe18eb496cff56b082a440e83bb19b8f2a02efed00f067d7ea4e18471d85` |
| E-0087-07 | `cargo +1.88.0 test --locked --all-targets --all-features --release` — all release targets passed. | `9e8cc8e5fe9056cc5740fd8edf443040b53a08ea27f02c878d2d7b95aaaafef7` |
| E-0087-08 | `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` — passed. | `acce857bddad44cbb5d34348d2141c3a7e32b2b271a822235617531a408ee6e5` |
| E-0087-09 | Focused CLI/residue tests — 1 CLI test and 3 residue tests passed. | `001c44bcc96a37a288ab2bff0cb66195673e0fe9ac970521b8f75c63bf9e18e6` |
| E-0087-10 | Merged MVP: fixture init/ingest plus `residue-report --backup`; four modes, eight artifact classes, one external backup, no report paths. | `e2c12c530404a921b0241e79324c801cb4b1b76dfed29b6a79414f93668179d0` |
| E-0087-11 | Roadmap, release-evidence, fixture, reproducibility, lifecycle, benchmark, identity, and Python validators — 46 tests; 160 tasks (66 done/94 backlog), 488 dependency edges, 108 parent edges, 36 measures, 12 milestones. | `89bcb986b1bee6e623dc0bfa60d833cf0c6f815add01b45a070ee249d467a57f` |

The MVP receipt also recorded `schema_version: 1`, `sqlite_secure_delete_enabled:
false` for the fixture journal, and a nonzero backup inventory. The report did
not serialize either journal or backup paths.

## Scope and limitations

- This task is read-only inventory and documentation. It does not delete rows,
  run `VACUUM`, destroy key material, remove backups, or promise recovery
  resistance.
- The sentinel matrix uses a real SQLite WAL/FTS5 fixture and synthetic sidecar
  files for classes that SQLite may remove before inspection. It records whether
  bytes remain after logical deletion; it intentionally does not generalize one
  filesystem or SQLite build into a universal erasure guarantee.
- `secure_delete` is reported as state, not as proof of erasure. External
  exports, snapshots, Time Machine/cloud copies, offline media, and privileged
  recovery remain separate responsibilities.
- The current product surface is the CLI and fixture path. A future UI must reuse
  the same four labels and unsupported-media disclosures before exposing any
  destructive operation.
