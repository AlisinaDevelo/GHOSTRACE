# Task 0085 evidence: export redaction preview and policy receipts

Task 0085 is implemented, reviewed, merged to protected `main`, and reproduced
on the named macOS device. Implementation PR [#293](https://github.com/AlisinaDevelo/GHOSTRACE/pull/293)
merged at `26a7ade9c039d3fec35079c73acd17d9540b49e9` on 2026-08-26. The
implementation commit before squash was
`28eba48df7b7a514b4a13db293c5b7a993da571a`. Issue #89 is closed only after
this evidence change is merged and linked.

## Contract and acceptance mapping

Plaintext export is now an explicit two-step declassification action. The
preview scans one ordered journal snapshot and retains only bounded labels,
counts, time ranges, policy identities, gap metadata, destination class, and
digests. It does not retain event payloads or the destination path. The v1
redaction inventory is the complete authorized event envelope; a future schema
version must add a true field projection before a narrower inventory can execute.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Preview and execution use one immutable query and redaction plan digest. | `ExportPlan` binds the query, v1 field inventory, policy id/version/scope digest, force choice, and destination class. Execution recomputes this plan and compares its digest before opening the publication path. `tests/export_preview.rs::preview_and_confirmed_execution_share_digests_and_hide_the_destination_path` and `filtered_query_is_bound_in_the_plan_and_manifest` pass. | PASS |
| Preview warns that plaintext metadata leaves encrypted storage and requires explicit confirmation bound to that plan digest. | `preview` prints the explicit plaintext warning; CLI `export` requires both `--confirm-plan` and `--confirm-snapshot`, and rejects an unconfirmed invocation before creating a destination. The vertical CLI test covers the negative path. | PASS |
| Policy or journal snapshot change requires reconfirmation or a bounded delta. | Policy identity/version/scope and the immutable plan are rechecked; a changed policy returns `ExportConfirmationMismatch`. A changed ordered journal snapshot returns `ExportSnapshotChanged` before publication. The tests verify no destination and no incomplete temporary remain in either case. | PASS |
| Receipt records destination class and manifest digest without retaining destination path in normal diagnostics. | `ExportReceipt` records plan, snapshot, and manifest digests, destination class, policy identity/version, and event count. Preview/receipt serialization tests and the merged CLI demo assert that the private destination path is absent. | PASS |

## Device reproduction and retained receipts

The exact merged SHA was rerun on the named device. Device facts are retained in
`/tmp/ghostrace-0085-device-facts.txt` with SHA-256
`1431f693bc958c1907da6198bf28e898875e1d77f78bbf7ceb3465d7047c16e0`.

| Fact | Recorded value |
| --- | --- |
| Source | protected `main` `26a7ade9c039d3fec35079c73acd17d9540b49e9` |
| Hardware | MacBookPro17,1, Apple M1, arm64 (`aarch64-apple-darwin`), 8 GB |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Rust/Cargo | rustc/cargo 1.88.0 (LLVM 20.1.5) |
| Python | 3.9.6 |
| Reproduction date | 2026-08-26 |

| Lane | Result | Retained log SHA-256 |
| --- | --- | --- |
| `scripts/reproducibility-test.sh` | exit 0; pinned fixtures/schema, deterministic and durable preview-to-confirmation exports, 46 Python tests, Clippy, and all Rust targets/features passed | `46d80587a3073225081aaef467929263299b8cccd0c131067ea39bbf14406da4` |
| `scripts/offline-network-test.sh` | exit 0; macOS network-denial canary, privacy regression, and complete offline all-target suite passed | `fe3993b38059049b220e9b0dbf66ca707ad792bed9002c270a5b7147f94d2210` |
| `scripts/fsevents-sanitizer.sh` | exit 0; nightly AddressSanitizer native lifecycle test passed without findings | `2a31ec686bcb221d7469396c316cdd13b9979c4c2846b73250e74a59738b73b1` |

The local pre-merge reproducibility lane also passed exit 0; its retained log
SHA-256 is `4134b159d534a0eb07a3e7891ec0cb18ce5a14681c441751d9437b982215ad2d`.
Generated `target/` output was removed narrowly before the sanitizer build when
the device had only a few hundred MiB free; no product test failed from that
resource event.

## MVP demo and resource receipt

On merged `main`, the direct device binary ran this flow using the checked-in
fixture:

```text
ghostrace preview --fixture fixtures/causal-chain.jsonl --output <private>/export.jsonl
ghostrace export --fixture fixtures/causal-chain.jsonl --output <private>/export.jsonl --confirm-plan <preview-plan-digest> --confirm-snapshot <preview-snapshot-digest>
ghostrace validate --export <private>/export.jsonl
```

The preview reported 8 events, plan digest
`sha256:f3715a3e5c85f1384bca0a0740644f3730b22e48e2d7e4c9cf3c5d608044b50b`,
snapshot digest
`sha256:83a11a7f56b5e3ff3a2495392e4a200c66423c60d2e4e08f203ee88c0a03a995`,
and the plaintext warning. Confirmation wrote 9 JSONL lines (one manifest and
eight events), 6,953 total bytes, mode `0600`, and validation reported
`validated 8 event(s)`. The full artifact SHA-256 is
`bbfca31a7425fc12560439c9c8f7662053776eacf44351a0da963ef24335b6ef`; the
manifest digest is
`sha256:e1b24f4f171774d7d0b285ffa49c9f1e845f369bb417048cc382255ffd406d4d`;
the event body is 5,602 bytes with digest
`83a11a7f56b5e3ff3a2495392e4a200c66423c60d2e4e08f203ee88c0a03a995`. The
`/usr/bin/time -l` receipt recorded 0.03 seconds real time and maximum resident
set size 11,878,400 bytes. No private path appeared in preview or export
diagnostics.

## Hosted review and merge

PR #293 was pushed from the implementation branch, all required hosted checks
were green (CI rustfmt/Clippy/Linux stable/Linux MSRV/macOS stable, offline
fixture, Cargo policy, dependency review, Rust advisories, and roadmap), and the
protected merge was a squash at `26a7ade9c039d3fec35079c73acd17d9540b49e9`.
The device lanes above are the acceptance evidence; hosted checks corroborate
the review gate but do not replace them.

## Boundaries and limitations

- The v1 plan displays and binds the complete authorized event field inventory;
  it does not yet implement a narrower field projection. A future schema and
  policy decision must precede selective field omission.
- Export is an intentional plaintext transition. The body spool and final
  artifact are private mode `0600` files and are atomically published, but the
  temporary spool is not encrypted and is removed on failure.
- The current CLI remains fixture-only with a deterministic test key. No live
  production Keychain source, ambient collector, or external destination was
  claimed by this task.
- The sanitizer evidence covers the checked-in native lifecycle boundary; it is
  not a substitute for release-scale soak, sleep/wake, logout, or volume-detach
  testing, which remain separate guarded rows.
