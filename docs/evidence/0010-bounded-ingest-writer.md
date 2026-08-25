# Task 0010 evidence: bounded ingest writer and atomic cursor commit

Status: complete on protected `main` at
`632f6b255bcea3923a7f461ba1d686beb15307d2` (PR #231). This receipt closes the
parent writer contract over the existing fixture journal. It does not enable
live collection, claim production throughput, or replace the explicit capture
refusal.

## Contract and implementation

`src/writer.rs` owns one FIFO worker and a bounded synchronous queue. Admission
is bounded by queue items, serialized request bytes, batch size, wait time, and
retry count. Queue pressure is visible as bounded backpressure, rejection, or a
source-labelled gap; there is no implicit drop. `Writer::submit` returns a
`WriteAck` only after `Journal::ingest_batch_with_diagnostics` commits.

The journal transaction writes encrypted event rows, the source cursor, the
policy reference, and bounded diagnostics together. Replays return the original
ingest sequence. A deterministic fault at `CursorBeforeUpdate` now proves that
an event inserted before the cursor write is rolled back with no acknowledgement;
the same event can then be retried and commits both event and cursor state.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Ingestion uses one writer and a bounded queue | `Writer` owns one worker and `sync_channel`; `WriterConfig::validate` enforces 64-item/4 MiB/16-event/250 ms/2-retry defaults and hard maxima. Queue depth and bytes are observable through `Writer::outstanding`. |
| Acknowledgement occurs only after commit | `process_request` sends `WriteAck` only after `ingest_batch_with_diagnostics` returns; `precommit_failure_returns_no_ack_and_rolls_back_event_and_cursor_together` receives the injected error instead of an acknowledgement. |
| Each event and its collector cursor commit transactionally | `insert_events` updates the cursor in the same SQLite transaction; the pre-commit fault test observes zero event rows and no cursor state after rollback, then verifies one event and its `last_event_id` after retry. |
| Queue saturation is observable | Deterministic writer tests cover `Reject`, bounded `Block` timeout, cancellation, and `EmitGap` with source and event count. No queue-full path silently discards a batch. |

## Device receipts

All receipts were run on 2026-08-25 from the protected merge on a MacBookPro17,1
(Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0, Rust/Cargo 1.88.0,
target `aarch64-apple-darwin`, with locked dependencies.

| Lane | Result and retained receipt |
| --- | --- |
| Source focused writer suite at `d53788e` | Pass: 9 tests; `/tmp/ghostrace-0010-source-writer.log`; SHA-256 `efd18d913c2fc64636f30a0db9ed2e89ffbd34bafdff69cc4a67b91b75aa68c2` |
| Protected-main focused writer suite | Pass: 9 tests; `/tmp/ghostrace-0010-postmerge-writer.log`; SHA-256 `9bfde12487c7a002a69f38fb69a1fc003e23ea487b7946b1be45d63b84a3aa34` |
| Protected-main debug all-target/all-feature suite | Pass; `/tmp/ghostrace-0010-postmerge-debug.log`; SHA-256 `f72cb8bf8a0ba90787d79a1bd4b59d08cbed51ed1fe350c555c079af49c07814` |
| Protected-main Clippy with warnings denied | Pass; `/tmp/ghostrace-0010-postmerge-clippy.log`; SHA-256 `1f2d831dcbcf174794e5be04b8f4810d1810ca342d9094b5edaadaad24970c78` |
| Protected-main macOS network-denial lane | Pass: denial canary, privacy regression, and complete product suite; `/tmp/ghostrace-0010-postmerge-offline.log`; SHA-256 `b406e144aef5578b0a407cb4b65169ec79742e876b3cfb80938d4dd04b63432a` |
| Protected-main release all-target/all-feature suite | Pass; `/tmp/ghostrace-0010-postmerge-release.log`; SHA-256 `146232822f37d2978dc30ad5ae611a7c2bb172c2ea7d0d4d5d9a9c1d477241a7` |
| Protected-main reproducibility/static pipe | Pass: pinned inputs, deterministic schema/demo/export, capture refusal, roadmap, 40 Python tests, Clippy, and complete Rust suite; `/tmp/ghostrace-0010-postmerge-repro.log`; SHA-256 `5db65ecc4d9d2f3cd8e0fe5a488e234216760122dd3c8a37f6c6253883709aa5` |
| Protected-main Rust documentation | Pass; `/tmp/ghostrace-0010-postmerge-doc.log`; SHA-256 `dd6c1b3a1db3fd883a21508756ac6158a0bf190715fd68bc58f047f1283b813d` |
| Protected-main shell/action lint | Pass; `/tmp/ghostrace-0010-postmerge-shellcheck.log` and `/tmp/ghostrace-0010-postmerge-actionlint.log`; both empty-success SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The hosted matrix for [PR #231](https://github.com/AlisinaDevelo/GHOSTRACE/pull/231)
was green across both duplicate CI runs, audit, Clippy, Cargo policy, dependency
review, offline fixture, roadmap, rustfmt, and Linux/macOS test jobs. The local
device lanes above are the acceptance evidence; hosted checks are an additional
merge gate.

## Scope limits

The evidence covers synthetic fixture ingestion and deterministic fault/retry
behavior. It does not claim live collector operation, event-storm capacity,
Intel validation, older macOS validation, signed/notarized distribution, or an
audit certification.

## Closure

Issue #14 can be closed against this evidence. The next live-collector gate must
measure its source-specific capacity and repair behavior independently.
