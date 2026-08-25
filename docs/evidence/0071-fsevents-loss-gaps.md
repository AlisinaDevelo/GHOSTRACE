# Task 0071 evidence: FSEvents loss and root-change gaps

Status: implementation and protected-main verification complete. The
implementation is merged at `0caed8b3bbce01219a73a9610c8db407b8c6b7ac`; this
evidence update is the follow-up receipt for that protected commit.

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

Protected-main implementation commit: `0caed8b3bbce01219a73a9610c8db407b8c6b7ac`
(PR [#254](https://github.com/AlisinaDevelo/GHOSTRACE/pull/254)). The hosted
post-merge workflows all passed for that exact commit: [CI run
32844178950](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32844178950),
[Rust advisories run
32844178813](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32844178813),
[offline fixture run
32844178783](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32844178783),
and [Cargo policy run
32844178749](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32844178749).

The exact merged commit was checked on the target device: macOS 26.6.2
(25G83), arm64, Rust 1.88.0, Cargo 1.88.0, and Python 3.9.6. Each command ran
against the checked-out protected commit; the receipt hashes below are SHA-256
of the complete stdout/stderr logs.

| Lane | Result | Receipt SHA-256 |
| --- | --- | --- |
| `cargo +1.88.0 fmt --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `RUSTFLAGS='-D warnings' cargo +1.88.0 clippy --all-targets --all-features` | passed | `17a283211fbaf930f9ab3cbca225f42112fe33f967c136a16893c9543890e227` |
| `cargo +1.88.0 test --all-targets --all-features` | passed; 31 unit tests and all integration targets, with one explicit Keychain authorization test ignored | `74b13fe4c23b67133621f34ae95a68e2fb11498a55d8281a3d7a2930ab3dca3a` |
| `cargo +1.88.0 test --release --all-targets --all-features` | passed; same explicit authorization exception | `4be2cc9e2800ed9bd843a7bd2aba8ca05bfa5db4f03a716205a56a2b7d8825b0` |
| `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --no-deps --all-features` | passed | `5393a91a2d15d3687022c8a92e25b19fbe2cbe7d36161378b9726b29bc2a9433` |
| `scripts/offline-network-test.sh` | passed; network-denial canary enforced | `244be1d54e73b7fd4f32d5550fe15df8bbf62fb9f00fd1f4e4633bc1bfa42ba7` |
| `scripts/reproducibility-test.sh` | passed; deterministic fixture/export/replay evidence | `74541f1099f76aaa8610212ae619a9e89d67c3ffcd19190b39880f06c23e92a6` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; 40/40 | `949bf3a3871bd4f6a001a727406688359484c412ffa9f22be2f786772a4cadcd` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 488 dependency edges, 0 blocked | `c3e7365cb85c75940fdaa46554563865bc7bea38512a486bfb4b3ebf36ab70f0` |

No path, display name, account data, credential, or capture key is retained in
the evidence. The ignored Keychain lifecycle test is an explicit device
authorization boundary, not a substituted or silently skipped coverage claim.
