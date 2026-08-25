# Task 0064 evidence: selected-root consent and lifecycle receipts

Status: complete on protected `main`; implementation merge
`af362e41c402a155fbad56a75882920b3d02752b` (PR #238) and final evidence readback
`13e627d3a8e289ecfd293761c5dcfe09e6297184` (PR #239). This child gate makes
selected-root consent inspectable before a future collector can be enabled; it does
not itself start a live source.

## Contract and implementation

`ConsentPreview::from_policy` produces a deterministic, bounded, user-visible summary
of canonical opaque root identities, exclusions, retained fields, coverage limits,
policy identity/version, and the policy scope digest. The preview rejects an empty
scope, missing retained-field declarations, missing coverage limits, duplicates, and
oversized lists. Raw filesystem paths are not accepted as root IDs.

`ConsentPreview::confirm` consumes the rendered preview into a non-reusable
`ConsentConfirmation`; `ConsentStateMachine::grant_preview` accepts only that private
confirmation payload. The receipt contains the policy identity/version and scope
digest, never root names, retained-field names, paths, or observations. `revoke`
returns a `revoked` terminal receipt synchronously and leaves capture disallowed before
cleanup can run.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| The user sees canonical root identity, exclusions, retained fields, and known coverage limits before enabling capture | `tests/selected_root_consent.rs::preview_makes_root_scope_and_known_limits_visible_before_explicit_confirmation` serializes the preview, checks deterministic sorted fields, and verifies that no raw `/Users/` path is present. |
| A receipt binds the root scope to an immutable policy version without storing path content in diagnostics | `confirmed_preview_binds_receipt_to_policy_without_retaining_scope_names_and_revoke_is_terminal` compares the receipt digest and policy version to the source policy and asserts the serialized receipt contains neither root IDs, retained-field names, nor paths. |
| Revocation stops observation and produces a bounded terminal status before the command returns | The same test requires `ConsentState::Revoked`, `is_terminal()`, a `Revoked` receipt, and `is_capture_allowed() == false`; a second revoke is rejected. |

## Device receipts

Receipts were run on 2026-08-25 on a MacBookPro17,1 (Apple M1, arm64), macOS 26.6.2
build 25G83, Darwin 25.6.0, Rust/Cargo 1.88.0, target `aarch64-apple-darwin`,
and Python 3.9.6.

| Lane | Result and retained receipt |
| --- | --- |
| Implementation commit before merge | `e21c852` on PR #238; source full local pipe passed |
| Source full reproducibility pipe | Pass: `/tmp/ghostrace-0064-source-repro.log`; SHA-256 `013368b60dddee9fabda56f69e440835cf065ba268917f713b53d1398f378acb` |
| Protected-main focused consent suite | Pass: 3/3; `/tmp/ghostrace-0064-postmerge-consent.log`; SHA-256 `b6ff1fec9b745866cb3c82d108440bc3a52ef87f7ab1819674fc28fc5679f00c` |
| Protected-main reproducibility pipe | Pass: pinned inputs, durable CLI path, 40 Python tests, locked Clippy, and all debug Rust targets; `/tmp/ghostrace-0064-postmerge-repro.log`; SHA-256 `4b231c58277a544f1e610da8bc0059ceacad59e7d8bda768c975555ab1390d01` |
| Protected-main release all-target/all-feature suite | Pass, including 3 release consent tests; `/tmp/ghostrace-0064-postmerge-release.log`; SHA-256 `92a290598ab7655bf4efa35469df25cb028948201611e7e363b1279841ee5075` |
| Protected-main rustdoc with warnings denied | Pass; `/tmp/ghostrace-0064-postmerge-doc.log`; SHA-256 `1f7da74d3483937255ed2fbaae32b30cfcc0a2ed03cb7b7b3c518b25fa3b7e14` |
| Protected-main macOS network-denial lane | Pass under `sandbox-exec`, including denial canary, privacy fixture, and complete product suite; `/tmp/ghostrace-0064-postmerge-offline.log`; SHA-256 `79db70da52ecee1fa76a9827ef3eff8144b8faa9161e03c7485f384c85314f93` |
| Hosted merge gates | PR #238 passed both CI runs, roadmap, audit, dependency review, deny, network-denial, rustfmt, Clippy, and Linux/macOS/MSRV lanes |
| Final protected-main focused readback at `13e627d3` | Pass: 3/3; `/tmp/ghostrace-0064-final-main-consent.log`; SHA-256 `59946437e7df52adb8117d1138e077ac928627048c7ee943e52c6f6fbd4244ff` |
| Final protected-main reproducibility readback at `13e627d3` | Pass: same complete local pipe; `/tmp/ghostrace-0064-final-main-repro.log`; SHA-256 `e7d52feea57d4c20e9ddb366cc070675ecd4efdb3d4858dc3ed41c6acf9822bb` |

The local device receipts are the acceptance evidence. Hosted checks are protected
merge gates, not a substitute for reproducing the behavior on the target device.

## Scope limits

This gate uses canonical opaque root identifiers from the versioned policy document;
it does not resolve filesystem paths, check APFS case/Unicode behavior, enforce
symlink or hard-link containment, persist cursors, or connect consent to the native
FSEvents stream. Those are separate gates (#0013, #0066–#0072). No live collector,
permission prompt, network client, or production capture key was enabled.

The existing `grant(&PolicyDocument, ...)` compatibility API remains for the fixture
state-machine tests. A live selected-root adapter must use the preview/confirmation
path before activation.

## Closure

Issue #68 can be closed against this receipt. Parent task #0013 remains open until
canonicalization, lifecycle wiring, persistence, and controlled filesystem integration
tests are complete.
