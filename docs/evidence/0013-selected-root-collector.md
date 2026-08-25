# Task 0013 evidence: selected-root macOS FSEvents collector

Status: complete on protected `main`. The implementation was merged by PR #241
at `8b4268676d5b59964ecb2ad65463c6f4b76b8ec4` on 2026-08-25. The final
protected-main receipts below are the acceptance record; hosted checks are merge
gates and are not a substitute for reproducing the behavior on the target device.

## Contract and implementation

`FseventsCollector::new` requires a confirmed consent receipt, an enabled
filesystem/lifecycle policy, and a selected-root set whose opaque IDs exactly
match the policy. A live collector instance is required, and the selected
directory is canonicalized before the native stream is created. The callback
path is copied only long enough to compute a bounded SHA-256 digest and map
FSEvents flags; the collector never opens, reads, or stats the changed path.

The collector owns an explicit bounded callback queue and a lifecycle state
machine (`Created`, `Running`, `Stopped`, `Revoked`, `Failed`). It records
started/stopped lifecycle events through the journal, exposes accepted/blocked/
dropped/coverage counters, maps controlled create/modify/move/delete callbacks
to path-free filesystem records, and turns queue overflow or incomplete native
coverage into explicit gap/diagnostic records. Revocation synchronously stops
the stream, clears pending callbacks, records the stop, and makes later starts
fail closed.

The implementation deliberately does not claim cursor persistence, volume
identity/reset/wrap recovery, exclusion precedence, symlink or hard-link open
races, storm/backpressure benchmarks, or an ambient command-line capture path;
those are later gates in the roadmap.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Root collection requires explicit opt-in | `FseventsCollector::new` accepts only `ConsentConfirmation`, verifies the policy receipt identity/version/digest, requires the filesystem and lifecycle sources, and rejects selected-root IDs that are not an exact policy mapping. `tests/selected_root_consent.rs` and `tests/selected_root_collector.rs` exercise the confirmation and scope checks. |
| File-level events are captured without content access | The native callback retains only a path digest, operation, entry kind, source event ID, and bounded evidence. The controlled macOS test writes a sentinel secret, then asserts the serialized records contain neither that secret nor the selected absolute path and that every digest has the `sha256:` prefix. No callback code opens, reads, or stats a path. |
| Collector lifecycle status is visible | `CollectorStatus` reports state, stream/consent state, accepted/blocked/dropped counts, coverage and callback health. The integration test observes `Created` → `Running` → `Stopped`, and journal assertions require `CollectorStarted` and `CollectorStopped`; the revocation test proves the terminal `Revoked` state and failed restart. |
| Controlled create, modify, move, and delete integration tests pass | `tests/selected_root_collector.rs::selected_root_collector_captures_controlled_file_lifecycle_without_content` drives a real selected directory on macOS, performs create/modify/rename/delete, and requires all four normalized operations. The protected-main focused receipt reports 3/3 tests passing. |

## Device and hosted receipts

Receipts were run on 2026-08-25 from the exact protected-main merge on a
MacBookPro17,1 (Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0,
Rust/Cargo 1.88.0, target `aarch64-apple-darwin`, and Python 3.9.6.

| Lane | Result and retained receipt |
| --- | --- |
| Protected-main focused selected-root suite | Pass: 3/3; `/tmp/ghostrace-0013-postmerge-collector.log`; SHA-256 `35c1ab71c335246de45624ea3898f5381b5dd2a89d3cbfa815e65d48e79c89dc` |
| Protected-main reproducibility pipe | Pass: pinned inputs, fixture manifest, rustfmt/schema, deterministic fixture CLI/export, capture refusal, roadmap/evidence checks, 40 Python tests, and all debug Rust targets; `/tmp/ghostrace-0013-postmerge-repro.log`; SHA-256 `a91eaaa41e623a6e0394efe404fa6cabfd849a757b45c04e4c68a92d0980e0a9` |
| Protected-main release all-target/all-feature tests | Pass: 23 library tests, every integration target including 3 selected-root collector tests, with the expected locked-keychain and network-canary ignores; `/tmp/ghostrace-0013-postmerge-release.log`; SHA-256 `690b3f4090d165cfd8ed6c68f94256f7d8f5ae5db701fe847d02c06a16a49b16` |
| Protected-main rustdoc with warnings denied | Pass with `RUSTDOCFLAGS='-D warnings'`; `/tmp/ghostrace-0013-postmerge-doc.log`; SHA-256 `f06b8d49c76e4f9f250208d1a288d6a46c7ca272902e201b4cc916dadf5037c4` |
| Protected-main macOS network-denial lane | Pass under `sandbox-exec`: denial canary, privacy regression, and complete offline locked product suite; `/tmp/ghostrace-0013-postmerge-offline.log`; SHA-256 `065dffede76a6456db373a3e7a8bb87eb89950ea5a7dd08884cac2eaaf63eb6f` |
| Hosted PR merge gates | PR #241 initially exposed Linux-only unused imports; portability fix `6f6fece` was pushed. Both hosted CI runs (`32831064757` and `32831068570`) then passed Linux stable/MSRV, macOS stable, Clippy, rustfmt, roadmap, Cargo policy, dependency review, advisories, and the network-denial fixture lane. |

The implementation branch also passed the focused collector suite, full locked
debug all-target/all-feature tests, Clippy with `-D warnings`, formatting,
roadmap, reproducibility, and fixture-manifest checks before merge. A local
x86_64 Linux cross-target attempt was not counted as a pass because this Mac
does not have `x86_64-linux-gnu-gcc`; the hosted Linux stable/MSRV lanes are the
cross-platform verification for this change.

## Privacy and scope limits

The test sentinel is synthetic and is never retained; no production path,
account data, credential, browser content, network client, or capture key was
used. The selected-root adapter is opt-in and path-free at the journal boundary,
but it is not yet a complete causal filesystem history: FSEvents coalescing and
native coverage loss remain explicit in the status and gap records. Cursor
durability, volume identity, reset/wrap handling, exclusion policy, race-resistant
open/containment checks, stress limits, and distribution hardening remain separate
gates.

## Closure

Issue #17 can be closed against this receipt. The parent task remains governed by
its remaining dependency gates; this child only establishes the first live,
selected-root metadata collector.
