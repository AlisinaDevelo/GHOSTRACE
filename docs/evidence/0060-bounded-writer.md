# Task 0060 evidence: bounded durable writer and acknowledgement semantics

Status: complete for the verified fixture writer contract. This task adds one
FIFO durable writer over the existing SQLite journal. It does not enable live
collection, claim production throughput, or replace the explicit capture refusal.

## Scope and implementation

Implementation PR [#203](https://github.com/AlisinaDevelo/GHOSTRACE/pull/203)
was authored as commit `aaa3ecf4a96ed8d496fcc0239015240de4e3d124` and merged to
protected `main` at `0c6bae9c5ebf4b7d91ea705fd346fd7c6b541238` on 2026-08-25.
The implementation is in `src/writer.rs`; `Journal::ingest_batch_with_diagnostics`
keeps event rows, cursor progress, the policy reference, and bounded diagnostics
inside one SQLite transaction before a `WriteAck` is sent.

The public configuration contract is:

| Bound | Safe default | Hard maximum |
| --- | ---: | ---: |
| Outstanding queue items | 64 | 4,096 |
| Events per batch | 16 | 1,024 |
| Serialized request memory | 4 MiB | 64 MiB |
| Admission wait | 250 ms | 30 s |
| SQLite busy retries | 2 (3 total attempts) | 8 |

`Block`, `Reject`, and `EmitGap` can be selected per `EventSource`. A gap carries
the source, event count, and `QueueFull` reason. Cancellation before worker start
returns `WriterCancelled`; after transaction start it cannot erase a committed
write. Diagnostic codes are bounded ASCII identifiers and details are limited to
512 non-control bytes, so payloads and paths cannot be smuggled into diagnostics.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Queue item, batch, memory, wait-time, and retry bounds are configuration contracts with safe defaults | `WriterConfig::default/validate`; `defaults_are_bounded_and_source_policy_is_explicit`; `fifo_acknowledgements_are_ordered_and_memory_is_bounded`; source and merged focused logs below |
| Acknowledgement follows atomic event, cursor, policy reference, and diagnostics commit | `Journal::ingest_batch_with_diagnostics`; `acknowledgement_follows_one_atomic_event_cursor_policy_and_diagnostic_commit`; ack contains request/source/event IDs, sequences, policy identity/version, diagnostic count, attempts, queue wait, and commit time |
| Full queues block, reject, or emit a source-specific gap without silent drop | deterministic `writer::tests::queue_full_policies_are_source_specific_and_explicit`; `writer::tests::block_policy_has_a_bounded_wait_and_queued_cancellation_is_visible`; explicit `WriterGap`; focused logs |

Negative and recovery coverage includes invalid configuration, empty/oversized and
mixed-source batches, oversized serialized memory, invalid diagnostic identifiers,
FIFO ordering, atomic metadata counts, bounded busy retry exhaustion, source-specific
reject/block/gap behavior, and queued cancellation. The standard suite also reruns
migrations, WAL checkpoint/reader limits, path permissions, privacy corpus, export,
schema, and capture refusal.

## Source-device pipe

The source run was executed from implementation commit `aaa3ecf4a96ed8d496fcc0239015240de4e3d124` on the target device:

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |
| Full fail-fast pipe | `/private/tmp/ghostrace-0060-source-pipe.1787617603.log` (651 lines) |
| Full pipe SHA-256 | `8fd587f067b8ef9f361984f386a14e52f797e135c26b3417e68d54fcc874ec2f` |
| Focused writer log | `/private/tmp/ghostrace-0060-writer-measurements.1787617713.log` (33 lines) |
| Focused log SHA-256 | `c0bc4660e9a4bf52fb08b81248fce6264bdc4a98c94de916bf9530d1ee6cde67` |

The full pipe ended `SOURCE_PIPE_PASS` after locked offline metadata, diff check,
format, all-target/all-feature check and Clippy (`-D warnings`), debug and release
all-target tests, doctests/docs, roadmap, 38 Python tests, reproducibility checks and
script, sandboxed network denial, ShellCheck, and actionlint. The focused writer
suite passed all 5 integration tests. Its `/usr/bin/time -l` sample was 0.41 s real,
60,243,968-byte maximum resident set, and 43,648,320-byte peak footprint. This is a
local smoke measurement, not a production capacity claim.

## Protected-main reproduction

The same clean-worktree reproduction ran from the protected implementation merge
`0c6bae9c5ebf4b7d91ea705fd346fd7c6b541238` with the same device, OS, architecture,
toolchain, locked inputs, offline setting, and one-job limit:

| Field | Value |
| --- | --- |
| Full fail-fast pipe | `/private/tmp/ghostrace-0060-merged-pipe.1787617734.log` (653 lines) |
| Full pipe SHA-256 | `085913c7963969b9ae91cd4bb504f9e4712b0b5d1a7b8704e93e4cbe5386f1b8` |
| Focused writer log | `/private/tmp/ghostrace-0060-writer-measurements-merged.1787617864.log` (33 lines) |
| Focused log SHA-256 | `7665d5d9881c86d95b9d11c271e8d8ba24fab0c1c169a050fb6570f82dde1fee` |

The merged full pipe ended `MERGED_PIPE_PASS` and repeated the source format,
check, Clippy, debug/release tests, docs, roadmap/Python, reproducibility, privacy,
fixture/identity, ShellCheck, actionlint, and sandbox network-denial lanes. The
focused writer suite again passed all 5 tests. Its resource sample was 0.40 s real,
59,932,672-byte maximum resident set, and 43,337,024-byte peak footprint.

Hosted PR checks were all green, but the retained acceptance decision is based on
the device pipes above. Intel macOS, the macOS 15 floor, signed/notarized
distribution, and live collectors remain explicit no-go or unverified scope.
