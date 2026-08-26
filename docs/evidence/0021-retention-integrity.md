# Task 0021: retention, deletion, and integrity

Status: complete on protected `main`.

This receipt covers the confirmed logical-deletion command and the read-only
integrity/recovery boundary. It does not claim secure erasure, compaction,
cryptographic key destruction, or removal of external copies.

## Delivery

- Issue: [#25](https://github.com/AlisinaDevelo/GHOSTRACE/issues/25)
- Implementation PR: [#306](https://github.com/AlisinaDevelo/GHOSTRACE/pull/306)
- Implementation commit before squash: `e7fb5da`
- Protected-main merge: `11811db6ef3d2c7474924855a340299ca32345cf`
- Reproduction date: 2026-08-26 UTC

`retention-delete` accepts only the plan digest, candidate-set digest, and
snapshot boundary from one dry-run. It re-evaluates the scope in an immediate
SQLite transaction, refuses drift, cursor-tail references, and unselected child
events, and deletes selected rows in reverse ingest order. Its receipt reports
exact counts and says compaction was not performed and external copies were not
touched. `integrity-check` runs bounded SQLite integrity and foreign-key checks
and returns recovery guidance without attempting repair.

## Acceptance evidence

| Evidence | Acceptance criterion | Result |
|---|---|---|
| E-0021-01 | The default retention period is documented. | `DEFAULT_RETENTION_DAYS` is 90, `RetentionPolicy::default_at` is explicit, README/privacy/architecture documentation states the default and gap-preservation behavior, and the focused test asserts the exact 90-day window. |
| E-0021-02 | Dry-run reports exact affected counts. | The existing authenticated retention plan remains read-only and binds snapshot boundary, candidate digest, affected count, sources, key generations, gaps, and estimated encrypted bytes; merged full and focused lanes pass it. |
| E-0021-03 | Deletion is scoped and transactional. | Focused tests pass exact confirmation deletion, stale-confirmation refusal, and unselected-child refusal with both rows unchanged. The CLI/repro lane also exercises a confirmed zero-candidate receipt. |
| E-0021-04 | Integrity checks and recovery guidance work. | The merged focused lane passes the healthy integrity report (SQLite `ok`, no foreign-key violations, schema/migration counts, four bounded recovery instructions); the CLI and reproducibility lanes compare deterministic receipts. |

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 (25G83)
MacBookPro17,1-class Apple Silicon / arm64
rustc 1.88.0 (LLVM 20.1.5), host aarch64-apple-darwin
Python 3.9.6
```

## Merged-main verification

Every receipt below ran from `11811db6ef3d2c7474924855a340299ca32345cf` in a
fresh evidence worktree. Digests are SHA-256 of retained local logs.

| Receipt | Command/result | Log SHA-256 |
|---|---|---|
| E-0021-05 | `scripts/reproducibility-test.sh` — all checks passed; locked Clippy, all debug targets, 46 Python tests, confirmed deletion receipt, and integrity receipt passed. | `5bb3ece241a3ec46e85127eb0beea6475043484436523661009aad943a308e63` |
| E-0021-06 | `scripts/reproducibility-test.sh --offline-network` — all checks passed with network denied. | `c35cd450971815e2d4bac566397182eb3b2e17ee9c9a85ed299070620acc2e01` |
| E-0021-07 | `scripts/fsevents-sanitizer.sh` — native lifecycle passed; three documented `*_fetchInitializingClassList*` suppressions. | `f1ae7edbe1d9ca0ddbe84765abe0d2db6e18b60d3d346240d26736a534d48e01` |
| E-0021-08 | `cargo +1.88.0 test --locked --all-targets --all-features --release` — all release targets passed. | `ed20dbe0e3c5d9384a99d623d6cb961a57eb5867697d8a87f12a991c23779ca5` |
| E-0021-09 | `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` — passed. | `02ac561fb38eb611a9e1be7cc83f1aea43cde2dcbb9099ad8991613c028c4562` |
| E-0021-10 | Focused CLI/retention tests — 1 CLI test and 9 retention tests passed. | `a714a8ab43309857e086d8f1955215b977f7c28ecebcd36f322bb1a16cbaffde` |
| E-0021-11 | Merged MVP: fixture init/ingest, zero-candidate confirmed deletion, integrity check, and residue report; receipt says 0/0 deleted, 8 remaining, integrity `ok`, four guidance entries. | `4d7d86959d8708b926b1b29d4f309fe7ea0becaa9e51f673cce630b21a9fe0ba` |
| E-0021-12 | Roadmap, release-evidence, fixture, reproducibility, lifecycle, benchmark, identity, and Python validators — 46 tests; 160 tasks (67 done/93 backlog), 488 dependency edges, 108 parent edges, 36 measures, 12 milestones. | `52bb26bacb89ee69b528d41facfcd7a7f0a643cc07959f16cce583937efb6758` |

## Scope and limitations

- Logical deletion is transactional and exact, but this task does not run
  `VACUUM`, compact free pages, destroy key material, or remove backups.
- Parent/child and durable cursor-tail references are refused rather than
  silently orphaned. A caller must plan a safe scope or perform a separate
  recovery decision.
- `integrity-check` is diagnostic. A failed check requires stopping ingestion,
  preserving the original database and sidecars, and working on a private
  verified copy with before-and-after receipts; it never auto-repairs.
- SQLite WAL/SHM, snapshots, Time Machine/cloud copies, offline media, SSD wear
  levelling, privileged recovery, plaintext exports, and independently encrypted
  copies remain separate residue responsibilities documented by task 0087.
