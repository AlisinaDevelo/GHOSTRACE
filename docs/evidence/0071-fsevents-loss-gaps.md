# Task 0071 evidence: FSEvents loss and root-change gaps

Status: implementation complete on the source branch; protected-main merge and
final device receipts are added after the implementation and evidence PRs merge.

## Contract

Selected-root streams request Apple's `WatchRoot` create flag. The normalized
FSEvents contract maps `UserDropped`, `KernelDropped`, `EventIdsWrapped`,
`RootChanged`, and `MustScanSubDirs` to stable gap reason codes. A combined user
and kernel drop is retained as its own explicit reason while the raw flag word
remains available in the normalized source evidence.

Every selected-root coverage gap stores only the filesystem source, the volume
fingerprint, sorted opaque root IDs, comparable cursor bounds when available,
and a bounded remediation (`rescan_selected_roots`,
`reconcile_selected_root`, or `reinitialize_stream`). No callback path,
display name, rescan listing, or completeness claim is retained.

After a source-loss gap, the collector exposes `recovery_required` and refuses
to admit ordinary filesystem events. A later reconciliation/restart stage must
clear that gate; this task does not claim full source-loss recovery.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Distinct reason codes for dropped, wrapped, root-changed, and subtree-scan flags | `tests/fsevents_loss_gaps.rs::coverage_flags_have_distinct_gap_reason_codes` and `NormalizedFseventsEvent::gap_reason_code` |
| WatchRoot is enabled and root replacement cannot be silent | `FseventsOptions::default` includes `FLAG_WATCH_ROOT`; `tests/selected_root_collector.rs::root_replacement_emits_a_bounded_gap_before_any_resume_claim` renames the selected root on macOS and observes a durable `fsevents_root_changed` gap |
| Gap metadata is bounded and path-free | `tests/fsevents_loss_gaps.rs::gap_payload_retains_bounded_recovery_context_without_paths`; event schema permits only volume digest, opaque roots, cursors, and remediation |
| No continuous-coverage claim resumes after loss | The root replacement test recreates the path and generates a later file event; no `FilesystemChanged` event is admitted and status remains `recovery_required` |

## Limits

The gap is an explicit boundary, not a rescan implementation. Durable restart
resume, invalid/wrapped cursor startup policy, and reconciliation evidence remain
task 0015 and task 0072 work. The locked-session Keychain lifecycle test still
requires explicit device authorization and is reported as unavailable rather
than substituted by hosted CI.

## Target-device verification

Final protected-main SHA, hosted checks, macOS device details, commands, and
SHA-256 receipts are retained below after merge. No path, display name, account
data, credential, or capture key is retained.
