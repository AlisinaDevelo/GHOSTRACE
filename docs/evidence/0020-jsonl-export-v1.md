# Task 0020 evidence: JSONL export v1 with manifest

Task 0020 is implemented, reviewed, merged to protected `main`, and reproduced
on the named device. The parent capability is composed of the reviewed child
changes for the schema registry ([#289](https://github.com/AlisinaDevelo/GHOSTRACE/pull/289)),
bounded streaming ([#291](https://github.com/AlisinaDevelo/GHOSTRACE/pull/291)),
and redaction preview ([#293](https://github.com/AlisinaDevelo/GHOSTRACE/pull/293)),
plus the ten-million-record scale lane ([#300](https://github.com/AlisinaDevelo/GHOSTRACE/pull/300)).
Their protected-main implementation SHAs are `4d6b758c37e86616893297602a666531a8e39eb0`,
`dab11f48b653090f05f9d24cced7fef3304bce7c`,
`26a7ade9c039d3fec35079c73acd17d9540b49e9`, and
`d81a3166169f8026610e965606661048f6c55246`, respectively. This document is
the retained acceptance record for issue #24.

## Contract and acceptance mapping

The export is a versioned manifest followed by event JSONL. The manifest records
format and policy identity, selected sources and time range, coverage counts,
collector status, explicit gaps, and an incremental event-body digest. The
writer visits one ordered decrypted event at a time, keeps policy and gap state
bounded, writes through a 64 KiB copy buffer, validates the complete temporary
before publication, and atomically renames it with mode `0600`.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Manifest includes version, policy, coverage, collector status, and gaps. | The schema registry and manifest validator require the versioned format, policy profiles, coverage counters, collector status, and bounded gap records. Child evidence 0083 and the merged export/validate MVP exercise the complete manifest. | PASS |
| Ten-million-record fixture export streams without materializing the result and stays within 64 MiB incremental resident memory. | PR #300 raises the event bound to 10,000,000 and adds `export::tests::ten_million_synthetic_fixture_records_stream_through_bounded_buffer`. The test generates deterministic valid envelopes, serializes and digests each through the same `ExportRun` writer and 64 KiB buffer into `io::sink`, retaining no result. The warm protected-main run passed in 17.21 s with maximum RSS 59,637,760 bytes (incremental scale-lane budget: 67,108,864 bytes). | PASS, with scope noted below |
| Plaintext writing identifies sources/time and requires explicit confirmation. | The CLI preview prints the selected sources and time range; export refuses without the explicit sensitive-plaintext confirmation. Redaction preview/policy receipts and the merged MVP exercise preview, refusal, confirmed export, and validation. | PASS |
| Existing destinations are never overwritten without explicit confirmation. | The atomic writer uses `persist_noclobber` by default and requires the forced/explicit confirmation path for replacement. Child evidence 0084 fault-tests destination preservation and cancellation/disk-full cleanup; the merged MVP writes to a new private destination. | PASS |

The scale lane deliberately tests the ten-million-record serialization/count/
digest path against `io::sink` so the test does not retain a multi-gigabyte
artifact. The durable publication, manifest validation, permissions, digest
comparison, and overwrite behavior are separately exercised on the complete
fixture export. No claim is made that a ten-million-record artifact was retained
on disk.

## Protected-main device reproduction

The exact implementation merge SHA was rerun on the named device. Combined
stdout and stderr are retained in `/tmp` and hashed here.

| Fact | Recorded value |
| --- | --- |
| Source | protected `main` `d81a3166169f8026610e965606661048f6c55246` |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Rust/Cargo | rustc/cargo 1.88.0 (LLVM 20.1.5) |
| Python | 3.9.6 |
| Reproduction date | 2026-08-26 |

| Lane | Result | Retained log SHA-256 |
| --- | --- | --- |
| warm ten-million-record scale test | 1 passed, exit 0, 17.21 s, max RSS 59,637,760 bytes | `292044a6e7ffe7f2d6563f0d6242748bb723cec3ca4ba1f6f2766ad07f4dda97` |
| full reproducibility pipe | all pinned-input, fixture, deterministic CLI, roadmap, Python (46 tests), Clippy, and Rust target/feature checks passed; exit 0 | `97c3f0f95b6a970f9134315298c72d6cc0c9fc84496464d8ab1189716dad51c4` |
| offline network-denial pipe | sandbox denial canary, privacy regression, and complete product suite passed; exit 0 | `22625b233e70eee662cc85a69cedf00b7ff15ef46027a2f9f45294fa88e177a3` |
| sanitizer pipe | native lifecycle test passed; six documented runtime suppressions; exit 0 | `63e9a912e9bf9f59879761a1928c5cf127af63afb18d39389b44064f963a3537` |

The first cold scale attempt recorded compiler resource use (maximum RSS
506,134,528 bytes) rather than product memory; it is not used for the 64 MiB
acceptance measurement. The warm rerun above is the product test receipt.

## MVP export receipt

The protected-main CLI flow ran preview, plan/snapshot parsing, confirmed export,
and validation:

```text
target/debug/ghostrace preview --fixture fixtures/causal-chain.jsonl \
  --output <private>/export.jsonl
target/debug/ghostrace export --fixture fixtures/causal-chain.jsonl \
  --output <private>/export.jsonl --confirm-plan <preview-plan-digest> \
  --confirm-snapshot <preview-snapshot-digest>
target/debug/ghostrace validate --export <private>/export.jsonl
```

The flow exited 0. It produced one manifest plus eight event lines (9 JSONL
lines), 6,953 bytes, mode `0600`, and the validator recomputed the manifest's
event-body digest and coverage successfully. The full
artifact SHA-256 is `bbfca31a7425fc12560439c9c8f7662053776eacf44351a0da963ef24335b6ef`;
the plan and snapshot digests are
`f3715a3e5c85f1384bca0a0740644f3730b22e48e2d7e4c9cf3c5d608044b50b` and
`83a11a7f56b5e3ff3a2495392e4a200c66423c60d2e4e08f203ee88c0a03a995`; the
manifest-line digest printed by the export command is
`e1b24f4f171774d7d0b285ffa49c9f1e845f369bb417048cc382255ffd406d4d`.
Elapsed time was 0.10 s and maximum RSS was 11,747,328 bytes. The combined
receipt is `/tmp/ghostrace-0020-merged-mvp.log`, SHA-256
`d248a26c9de420e66aad60a7ccd9c395b3fcf808f5204191ea7d089aea3ece2f`.

## Closure validators

| Validator | Result | Log SHA-256 |
| --- | --- | --- |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | 46 passed, exit 0 | `9ec3c423dc4ea8d5f63fea1755b4478d3560545e5a37fecbd40610dda937c92e` |
| `python3 scripts/roadmap.py check` | 160 tasks, 12 milestones, 488 dependency edges, 108 parent edges; 64 done, 96 backlog, exit 0 | `c14abbd9b7a62c67a41c05ee4314596a3034002fa9cd6aab400317f96af36e75` |
| `python3 scripts/release-evidence.py check` | 36 measures, 12 milestones, exit 0 | `82ff0862f41c422cc4ad89be1d9fb40f64e2fda7179a690ffadff32994edc3f7` |

The roadmap validator was rerun after the task status was indexed on this branch;
the receipt and updated 64-done/96-backlog counts above keep the evidence file
and generated task ledger in lockstep.

## Boundaries and limitations

- The export boundary is fixture-backed; no live collector or production
  Keychain source is claimed by this task.
- Plaintext is intentionally emitted only after preview and confirmation and is
  protected by mode `0600`; the temporary body spool is not encrypted and its
  incomplete prefix is the crash-recovery marker.
- Policy profiles and gap records are bounded; records or metadata beyond the
  documented limits are refused rather than silently truncated.
- Native sleep/wake, logout, volume detach, and live Keychain authorization are
  explicit no-go scenarios in this evidence record.
- The M3 aggregate release gate and production readiness remain separate future
  decisions.
