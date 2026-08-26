# Task 0086 evidence: deterministic retention planning and dry-run

Task 0086 is implemented, reviewed, merged to protected `main`, and reproduced
on the named device. The implementation PR is [#302](https://github.com/AlisinaDevelo/GHOSTRACE/pull/302),
merged as `1a4c88ca8f7f4315ea4c12dd465f7f8bdb9d9dc6`. The implementation commit
before the protected-branch squash is `3c43b384d799569ad1167e5f6957c19a0d357743`.
This document is the retained acceptance record for issue [#90](https://github.com/AlisinaDevelo/GHOSTRACE/issues/90).

## Contract and acceptance mapping

`Journal::retention_plan` is read-only and evaluates one SQLite read snapshot.
`RetentionPolicy` intersects optional source and opaque-root scope, protects gap
records by default, and unions the explicit time, newest-event-count, and
newest-encrypted-byte selectors. The oldest eligible rows are selected in stable
`observed_at`, `ingest_seq`, and `event_id` order. The plan reports the committed
snapshot boundary, scoped and eligible counts/bytes, affected observed and
ingested ranges, source and ciphertext key-generation counts, bounded gap
summaries, selection reasons, and a conservative encrypted-payload byte estimate.
The candidate-set digest covers the selected event identity and storage metadata,
not payloads or paths. `RetentionConfirmation` binds the plan digest, candidate
digest, and snapshot boundary for the future destructive gate.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Dry-run reports affected counts, ranges, sources, key generations, gaps, and estimated reclaimed space. | The `retention-plan` CLI and public API emit all fields above. The merged device receipt reports snapshot/scoped count 8, eligible count 7, affected count 7, observed/ingested range `2026-01-01T00:00:00Z` through `2026-01-01T00:00:07Z`, sources filesystem 1/shell 1/git 1/browser 2/lifecycle 2, key generation 1 count 7, one protected gap, and 1,353 encrypted-payload bytes. | PASS |
| Policies define precedence and never silently treat an export as a backup or legal hold. | Policy validation rejects an unbounded empty policy and incompatible root/source scopes. Tests cover time, event-count, and byte-limit reasons and default gap protection. The plan explicitly lists exports, database backups/sidecars, and legal holds as non-goals; privacy documentation states that none is inferred or selected. | PASS |
| Concurrent ingest and policy updates cannot expand the deletion set after confirmation. | The plan binds a committed `ingest_seq` boundary and candidate-set digest. The retention tests ingest a new event after confirmation, prove the new plan has a different boundary/count/digest, and prove the old confirmation does not match it; a changed policy also changes the plan digest. The destructive deletion command is intentionally not part of this task and must enforce `matches_confirmation` before any mutation. | PASS for the planning/confirmation gate; deletion remains a parent-task gate |

The plan is a dry-run, not a deletion command. It does not delete rows, compact
SQLite, remove WAL/SHM sidecars, manage exports or backups, or implement legal
holds. `estimated_reclaimed_bytes` is the encrypted payload-byte lower bound, not
a promise of filesystem shrink after page reuse or external copies.

## Protected-main device reproduction

The exact implementation merge SHA was rerun on the named device. Combined
stdout and stderr are retained in `/tmp` and hashed here.

| Fact | Recorded value |
| --- | --- |
| Source | protected `main` `1a4c88ca8f7f4315ea4c12dd465f7f8bdb9d9dc6` |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| OS | macOS 26.6.2, Darwin 25.6.0 |
| Rust/Cargo | rustc/cargo 1.88.0 (LLVM 20.1.5) |
| Python | Python 3.9.6 |
| Reproduction date | 2026-08-26 |

| Lane | Result | Retained log SHA-256 |
| --- | --- | --- |
| full reproducibility pipe on merged main | all pinned-input, fixture, deterministic CLI, retention dry-run, roadmap, Python, Clippy, and Rust target/feature checks passed; exit 0 | `aa186ce08419d9f9dd18513d782c78bf534832ad8f82bcbfee5512d80476959e` |
| offline network-denial pipe on merged main | sandbox denial canary, privacy regression, and complete product suite passed; exit 0 | `1d03c47a7ebd0d140179e5038e845ecaf6f3ba67d19e804ec986c7acfc5e2192` |
| macOS sanitizer pipe on merged main | native lifecycle test passed; six documented runtime suppressions; exit 0 | `2aef4af84e4976b4a49a3383b8586b5735b3effd81252609a01838415778cac6` |
| release all-target/all-feature tests on merged main | no failure markers; all observed test results passed; exit 0 | `8b96f38286f215be502441e6d642ca93926316424054b7ef8d8f3d72b93591c7` |
| rustdoc with `-D warnings` on merged main | passed; exit 0 | `64ee073cd25b05667abf6fcbef7955fa23fb58b850ab8b5048bf09eb52015330` |

The local implementation-tree lanes were also green before review: full pipe
`65ea1626ba06cc19bdd0441b18e61fa1a80adad37251bae7cdae5c6bb53270b1`, offline
pipe `e6598074211573d3cc64b49571d25ceb24de15f47d954f1183e9d9ae3aa29c62`,
sanitizer `b5b275f30b93dfd70a9c0440b4cfa2a4c5535eb18726153537d3b10c9b0fce28`,
release `7b2f662280d80d546b854c68ccca3fba2eddbd6dbd3773326c88981f52bac8e9`,
and rustdoc `ba7440a93d03b4aec127dde412276f16e19ae9dfbc18af9e687d8a51e7ab6d27`.

## Merged-main retention receipt

The merged-main MVP initialized a private fixture journal, ingested eight events,
and ran the explicit cutoff `2026-01-01T00:00:08Z`. The combined receipt is
`/tmp/ghostrace-0086-merged-mvp.log`, SHA-256
`d5a775ebb3bc82350e44c8d56104c5cd44e01223ab15b7881fb9d313890acda8`. It is
byte-identical to the implementation-tree receipt because the fixture and
deterministic key seed are unchanged. The plan digests are
`sha256:f11e2c062e6eb7fc67b88af7ecf6b0b53881437e0a73339c3086477c3797a345`
and `sha256:bdcf36c80c76a047b5bf4ef761f39aa39709bb4fab81b03f4547b39812dad291`.
The command reported snapshot boundary/count 8, scoped count 8, eligible count
7, affected count 7, protected gap count 1, key generation 1 count 7, and
estimated encrypted payload bytes 1,353. No plaintext artifact was retained;
the temporary journal directory was mode 0700 and removed after the receipt.

## Closure validators

The validators were rerun after this task status was indexed:

| Validator | Result | Log SHA-256 |
| --- | --- | --- |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | 46 passed, exit 0 | `efa74a2eb2e110921200e75986f2c4d24d75963643250d8e3d267dfade07b4a4` |
| `python3 scripts/roadmap.py check` | 160 tasks, 12 milestones, 488 dependency edges, 108 parent edges; 65 done, 95 backlog, exit 0 | `55023aecc19ed4b1429f1b8560311d775bcf9181eca269f955f8e9d1e7d27ed9` |
| `python3 scripts/release-evidence.py check` | 36 measures, 12 milestones, exit 0 | `82ff0862f41c422cc4ad89be1d9fb40f64e2fda7179a690ffadff32994edc3f7` |
| `git diff --check` | clean, exit 0 | — |

## Boundaries and limitations

- The fixture path is offline and synthetic; no live collector, Keychain source,
  deletion, or production retention scheduler is claimed by this task.
- The future destructive command must replan or verify the confirmation boundary
  and candidate digest inside its own transaction before deleting anything.
- The byte estimate excludes SQLite page reuse, WAL truncation, backups, exports,
  sidecars, diagnostic records, cursors, and key references.
- The sanitizer uses the repository's documented system-runtime suppressions;
  no suppression hides a retention assertion.
