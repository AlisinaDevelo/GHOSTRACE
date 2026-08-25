# Task 0011 evidence: fixture replay and crash-injection harness

Status: complete on protected `main` at
`252805ee381316935ac8ef38c12cb6be6abb1636` (PR #233). This receipt proves the
fixture replay and writer crash boundary without enabling live collectors.

## Contract and implementation

`tests/replay_harness.rs` adds two executable parent-level checks. The replay
case ingests the checked-in causal JSONL corpus into two independent journals
using the same deterministic key and policy, compares the reports and serialized
event envelopes, verifies the lifecycle, shell, Git, filesystem, and browser
source set, and requires the omitted interval to remain a first-class gap.

The crash case launches its own test binary as a child, configures the writer to
abort at `IngestBeforeCommit`, and only writes an acknowledgement marker if the
writer returns. The parent requires the child to fail, the marker to be absent,
the reopened journal to contain no event or cursor, and a post-restart writer
retry to commit one event with its cursor. Existing vertical-slice tests cover
frontmost-application-shaped envelopes; the causal corpus remains synthetic.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Deterministic multi-source fixtures replay identically | `multi_source_fixture_replays_byte_identically_and_preserves_explicit_gap` compares two reports and byte-serialized event vectors from independent deterministic journals; it asserts the five corpus source classes and eight stable sequences. |
| Injected crashes create no false acknowledgements | `injected_writer_crash_produces_no_ack_and_replays_after_restart` aborts a child writer before commit, asserts the acknowledgement marker is absent, reopens with zero events/cursor, then retries and observes a committed `WriteAck`. |
| Any loss becomes an explicit gap | The replay report requires the known omitted-window event ID in `gap_event_ids` and exactly one stored `EventKind::Gap`; no missing interval is silently inferred. |

## Device receipts

All receipts were run on 2026-08-25 from the protected merge on a MacBookPro17,1
(Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0, Rust/Cargo 1.88.0,
target `aarch64-apple-darwin`, with locked dependencies.

| Lane | Result and retained receipt |
| --- | --- |
| Source focused replay/crash harness at `e685d78` | Pass: 2 tests; `/tmp/ghostrace-0011-source-replay.log`; SHA-256 `2a9c3093e88f624472f204593587519aae1b4fae8b80dc9389a765574e0debcf` |
| Protected-main focused replay/crash harness | Pass: 2 tests; `/tmp/ghostrace-0011-postmerge-replay.log`; SHA-256 `9da5d0b277d6d8d14e5c433de6ff4f2846d5c57612f853eef451262990dad7b1` |
| Protected-main debug all-target/all-feature suite | Pass; `/tmp/ghostrace-0011-postmerge-debug.log`; SHA-256 `dc9934c9b9eee3d1eb2a7fdfc73f24ddad7e8e8a9cce6b8533079e5e883a5b3e` |
| Protected-main Clippy with warnings denied | Pass; `/tmp/ghostrace-0011-postmerge-clippy.log`; SHA-256 `27ce3eda9112b7af37c8329152d8f309385573493027e9f51a873f3ab2329120` |
| Protected-main macOS network-denial lane | Pass: denial canary, privacy regression, and complete product suite; `/tmp/ghostrace-0011-postmerge-offline.log`; SHA-256 `443867fb45a2a8c804e631773cd2d4827b6f49da8f1d59c1612ec46b33c226d4` |
| Protected-main release all-target/all-feature suite | Pass; `/tmp/ghostrace-0011-postmerge-release.log`; SHA-256 `2049e43d6ae38054dc8aba6ff3c8606191421f2ac35f56fb42b6bb722a9559c4` |
| Protected-main reproducibility/static pipe | Pass: pinned inputs, deterministic schema/demo/export, capture refusal, roadmap, 40 Python tests, Clippy, and complete Rust suite; `/tmp/ghostrace-0011-postmerge-repro.log`; SHA-256 `556b0618d39103121658bab525a7e51ed2182b6be482cfa38fe758295b7e1edd` |
| Protected-main Rust documentation | Pass; `/tmp/ghostrace-0011-postmerge-doc.log`; SHA-256 `fc25d0dd9a86915773251130fd4517b5479b2daff55b238058163733a94bbb23` |
| Protected-main shell/action lint | Pass; `/tmp/ghostrace-0011-postmerge-shellcheck.log` and `/tmp/ghostrace-0011-postmerge-actionlint.log`; both empty-success SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The hosted matrix for [PR #233](https://github.com/AlisinaDevelo/GHOSTRACE/pull/233)
was green across both duplicate CI runs, audit, Clippy, Cargo policy, dependency
review, offline fixture, roadmap, rustfmt, and Linux/macOS test jobs. The local
device lanes above are the acceptance evidence; hosted checks are an additional
merge gate.

## Scope limits

The evidence covers synthetic fixtures, deterministic replay, explicit gap
propagation, and a child-process crash before SQLite commit. It does not claim
live collector recovery, event-storm capacity, Intel validation, older macOS
validation, signed/notarized distribution, or audit certification.

## Closure

Issue #15 can be closed against this evidence. Live source repair and recovery
remain separate gates.
