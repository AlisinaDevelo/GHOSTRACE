# Task 0007 evidence: consent and capture-policy engine

Status: complete for the fixture-only product contract. This gate does not enable
live collection, permissions, telemetry, cloud sync, or a network client.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Consent is deny-by-default | `PolicyProfile::deny_by_default` starts with no enabled sources, no selected or excluded roots, and private context disabled. The consent machine allows capture only in `Active`; failed commands leave state unchanged. The deterministic consent corpus passed 256 sequences of 96 commands. |
| Selected roots, exclusions, private context, and redaction decisions are enforced | `PolicyProfile::decide` checks source, selected root, exclusion precedence, and private context before authorization. `root_excluded` is a stable denial reason; bounded decision records expose only root presence and `redact`/`summarize` dispositions. The policy corpus passed 512 generated scope matrices and asserted no rejected root string was serialized. |
| Decisions are versioned and reason-coded | Policy documents and runtime profiles carry an immutable identity and positive version. Scope digests include both root sets, and exclusion changes produce `PolicyChange::ExcludedRoots` requiring reconfirmation. Decision records and consent receipts retain stable reason codes without sensitive values. |
| Property tests cover the consent and policy state machine | `tests/policy_state_machine.rs` is a deterministic, dependency-free corpus covering policy round trips, digest stability, deny precedence, redaction, migration, replay, monotonic receipts, failed-command immutability, and rejection of forged non-grant reactivation. |

## Target-device receipt

All receipts below were run from clean protected `main` commit
`45f63aa2780fef4ff81962e1ec978cf2098d5af5` on 2026-08-25 on the target device:

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin` |
| Deterministic property corpus | `/private/tmp/ghostrace-0007-property-v4.log`; SHA-256 `3802d7c0dccc3c42d3ce7794c6331c689e371bca93d7cb8d294e833f631accff` |
| Complete debug all-target/all-feature suite | `/private/tmp/ghostrace-0007-debug-all-v2.log`; SHA-256 `5518c925d50218213acb5fa27d21d78f06e211a200d951e0800bab3dbe90967c` |
| Enforced macOS sandbox lane | `/private/tmp/ghostrace-0007-offline-v1.log`; SHA-256 `dbbdf112bcf7bbba035a7915cfe7c9fcb45cf1a8f5876c390831070c9976fa14` |
| Static and repository checks | `/private/tmp/ghostrace-0007-static-v2.log`; SHA-256 `839bf0f27bfb60608196f2251f2993984259b82aa1adb3f195b504192b8b64eb` |
| Complete release all-target/all-feature suite | `/private/tmp/ghostrace-0007-release-all-v2.log`; SHA-256 `6086966289fffde994f4f3485756b40abdc1131078ea75e3b7119e78d69f4b4f` |

The debug and release suites passed the library, migration, vertical-slice, privacy,
writer, WAL, FSEvents lifecycle, and state-machine tests. The release migration
suite intentionally prints failed child processes for crash-injection cases; the
parent recovery assertions pass and the command exits zero. The enforced lane
passed the network-denial canary, privacy regression, and complete debug suite.
Static checks passed formatting, Clippy with `-D warnings`, fixture/identity/schema
checks, roadmap/index validation, 38 Python tests, ShellCheck, actionlint, and the
product network-surface scan.

## Boundaries and limitations

- The product remains fixture-only. No live FSEvents collector, TCC permission,
  journal export, telemetry path, or network client was enabled by this task.
- The local Docker daemon is unavailable. The checked-in Linux workflow remains the
  enforcement authority; the receipt above uses the target macOS `sandbox-exec`
  equivalent and does not claim a Docker result.
- No real path, account name, credential, browser content, or event payload was used.
- The first debug receipt for this branch hit the device disk limit and was rerun with
  task-owned targets removed and one build job; only the passing receipts above are
  acceptance evidence.

## Post-merge closure rerun

The implementation was squash-merged in PR #213. The receipts below were rerun from
the exact protected `main` commit
`7af88387f4d91b8499059e6ceffe4c6edaa1f9dd` on the same target device:

| Receipt | Result | SHA-256 |
| --- | --- | --- |
| `/private/tmp/ghostrace-0007-postmerge-release-v1.log` | Full locked release all-target/all-feature suite, including 16 library, 5 cursor, 4 fault, 1 FSEvents, 7 migration, 2 policy property, 1 privacy, 5 support, 26 vertical, 6 WAL, and 5 writer tests: pass, exit 0 | `b024ef0dcdf43f01d8bda0db3f46e7b8ac1bf40b3ccf4dc20a6c627f2da33f58` |
| `/private/tmp/ghostrace-0007-postmerge-sandbox-v1.log` | Direct release network-denial canary under macOS `sandbox-exec`, plus focused release privacy and policy-property tests: pass, exit 0 | `e902a4474e5caa5b797650d4f05170878382edc6f0564e65a57b3ef391ccb1f2` |
| `/private/tmp/ghostrace-0007-postmerge-static-v1.log` | Format, locked release Clippy (`-D warnings`), fixture/identity/release-evidence/roadmap/reproducibility checks, 38 Python tests, index parity, ShellCheck, actionlint, and product network scan: pass, exit 0 | `97b3cf3dd51530102915bac0b3e899e001fa9e8c86b579d87977821d14be399b` |

The release migration test prints expected failed crash-child processes while the
parent recovery assertion passes. The separate post-merge PR contains documentation
only; no product source, fixture, workflow, or test input changed after PR #213.
Hosted protected checks for PR #213 also passed, including the retried Linux MSRV
job after one timing-sensitive WAL test failure.
