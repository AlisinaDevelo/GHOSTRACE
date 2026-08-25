# Task 0084 evidence: stream exports through an atomic bounded writer

Task 0084 is implemented, reviewed, merged to protected `main`, and reproduced
on the named device. Implementation PR [#291](https://github.com/AlisinaDevelo/GHOSTRACE/pull/291)
merged at `dab11f48b653090f05f9d24cced7fef3304bce7c` on 2026-08-25. The reviewed
implementation commit before squash was
`8b0b136ff5db1ded709c78f7c62de7a4c9d5554f`. Issue #88 is closed only after
this evidence change is merged and linked.

## Contract and acceptance mapping

The export format remains the strict v1 manifest-plus-event JSONL contract from
task 0083. Task 0084 changes the execution path: `Journal::for_each_ordered_event`
holds a read snapshot and visits one decrypted event at a time; a private body
spool records one bounded JSON line per visit and updates counts and SHA-256
state incrementally. A second private temporary writes the manifest and copies
the body in 64 KiB chunks. The complete temporary is validated before it can be
renamed into place.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Records stream in stable order with bounded buffers and incremental manifest digests. | `Journal::for_each_ordered_event` uses the shared `(observed_at, ingest_seq, event_id)` SQL order without collecting a `Vec<StoredEvent>`. `ExportStats` retains only one encoded record plus bounded policy/gap metadata, and updates body bytes and SHA-256 per record. `MAX_EXPORT_RECORD_BYTES=1 MiB`, `MAX_EXPORT_EVENT_RECORDS=1,000,000`, `MAX_EXPORT_POLICY_PROFILES=4,096`, and `MAX_EXPORT_GAPS=4,096` are enforced by both writer and validator. `tests/export_streaming.rs::successful_streaming_export_remains_fully_validated` and the merged reproducibility lane pass. | PASS |
| Temporary creation, permissions, fsync, rename, cancellation, disk-full, and existing-destination behavior are fault-tested. | Body and final files use explicit `.ghostrace-export-incomplete-*` prefixes, mode `0600`, `sync_all`, pre-rename validation, same-directory `persist`/`persist_noclobber`, and directory fsync. `tests/export_streaming.rs` covers successful publication, pre-cancelled export, forced-export destination preservation, and bounded-line rejection. Export unit tests cover cancellation after one streamed record and simulated disk-full during final copy while preserving the old destination and removing both temporaries. Existing overwrite and private-mode coverage also remains in `tests/vertical_slice.rs`. | PASS |
| Partial output is removed or unmistakably marked incomplete and never carries a valid final manifest. | Every error path drops both `NamedTempFile`s before publication; cancellation is checked before each event, body copy chunk, and rename. Temporary names are explicitly marked incomplete, and the final temporary must pass `validate_export` before rename. Mid-stream cancellation leaves no destination or temporary; disk-full leaves the prior destination byte-identical. | PASS |

## Device reproduction and retained receipts

The exact merged SHA was rerun on the named device. The device facts receipt is
`/tmp/ghostrace-0084-device-facts.txt` (SHA-256
`9ec380b6a924a4553c8d0ab302ba3b81c244e01cfaffe477b0d1781ed67004d7`).

| Fact | Recorded value |
| --- | --- |
| Source | protected `main` `dab11f48b653090f05f9d24cced7fef3304bce7c` |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Rust/Cargo | rustc/cargo 1.88.0 (LLVM 20.1.5) |
| Python | 3.9.6 |
| Repro date | 2026-08-26 |

| Lane | Result | Retained log SHA-256 |
| --- | --- | --- |
| `scripts/reproducibility-test.sh` | all pinned-input, fixture, deterministic CLI, roadmap, Python (46 tests), Clippy, and Rust target/feature checks passed; exit 0 | `2db31faf04ba222835b1ed9e9ecd5077553b1280dbbaa4ea9d3a738820fcc680` |
| `scripts/offline-network-test.sh` | sandbox network-denial canary, privacy regression, and complete product suite passed; exit 0 | `fed89e22860f70471f7bd39f2652ed0ca17b81c6edc2816653113243607dd91b` |
| `scripts/fsevents-sanitizer.sh` | nightly AddressSanitizer native lifecycle test passed; exit 0 | `54b950fcd8882f9adfd9b353da021eb897f962ed8d78a832ef0c4fb9f5625bf5` |

The first sanitizer attempt returned `ENOSPC` while the ignored task worktree
had accumulated approximately 2.0 GiB of generated `target/` output and only
133 MiB remained. The generated directory was removed narrowly after checking
for active build processes; the retry above passed. This is retained as a
device-resource event, not a product failure.

## MVP demo and resource receipt

On merged `main`, the CLI demo ran:

```text
target/debug/ghostrace export --fixture fixtures/causal-chain.jsonl --output <private>/export.jsonl
target/debug/ghostrace validate --export <private>/export.jsonl
```

Both exited 0 and printed `exported 8 event(s)` and `validated 8 event(s)`.
The output had 9 JSONL lines (one manifest plus eight events), mode `0600`,
6,953 total bytes, 5,602 event-body bytes, `coverage.event_count=8`, and
`coverage.gap_count=1`. The manifest event digest and independently recomputed
body digest were
`83a11a7f56b5e3ff3a2495392e4a200c66423c60d2e4e08f203ee88c0a03a995`; the full
file SHA-256 was
`bbfca31a7425fc12560439c9c8f7662053776eacf44351a0da963ef24335b6ef`.

The direct merged binary resource run completed in 0.11 seconds with maximum
resident set size 11,632,640 bytes. Its output had the same full-file digest;
the receipt is retained under `/tmp/ghostrace-0084-resource.BAhxys/`.

## Hosted review and merge

PR #291 had 19 successful hosted check entries across the push and pull-request
runs: CI (rustfmt, Clippy, Linux stable/MSRV, macOS stable), offline fixture,
Cargo policy, Rust advisories, dependency review, and roadmap. The observed
workflow run IDs included `32910474752`, `32910496584`, `32910496551`,
`32910496553`, `32910496596`, and `32910496626`; the merge state was `CLEAN`
before squash. Hosted checks were a review gate; the device lanes above are the
acceptance evidence.

## Boundaries and limitations

- The explicit bounds keep memory bounded but reject records or metadata beyond
  those limits; raising them requires a future contract decision.
- Export is an intentional plaintext transition protected by mode `0600` and
  atomic publication. The temporary body spool is not encrypted, and its
  incomplete prefix is the crash-recovery marker.
- The current repository remains fixture-only: no live collector or production
  Keychain source is enabled. This task proves the authorized `Journal` export
  boundary and durable fixture CLI path, not native source availability.
- Disk-full was fault-tested deterministically at the writer boundary; no
  destructive full-disk test was attempted. Native sleep/wake, logout, and
  volume-detach rows remain explicit no-go or ignored scenarios.
