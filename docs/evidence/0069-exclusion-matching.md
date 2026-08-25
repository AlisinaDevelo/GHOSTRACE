# Task 0069 evidence: versioned exclusion precedence and matching rules

Status: complete for the fixture-only product contract. This gate defines a
pre-persistence policy engine; it does not enable live collection, permissions,
telemetry, cloud sync, or a network client.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Deterministic precedence covers allow, deny, redact, and summarize | `ExclusionPolicy` ranks safety action (`deny > redact > summarize > allow`), then rule class (user > subtree > root > application > file kind > temporary file > VCS), then literal specificity. Input order is not a tie-break. Tests cover every rule class and all outcomes. |
| Updates affect future events and preserve recorded versions | `ExclusionPolicyHistory` retains validated versions, evaluates new subjects through the current version, and can explicitly evaluate a recorded subject against its original version. Equal and decreasing versions are rejected. The test proves a v1 deny remains unchanged after v2 is installed. |
| Property coverage and bounded matching | `tests/exclusion_policy.rs` covers overlapping/nested rules, case variants, escaped wildcards, malformed and empty patterns, order-independent digests, version history, schema round trips, and 512 maximum-size matching iterations. The matcher is greedy linear-time, with 128-rule/128-byte-pattern/1024-byte-subject bounds and no regex backtracking. |

## Rule and privacy contract

The engine evaluates an ephemeral subject containing root identity, relative path,
file kind, application identifier, temporary-file flag, and VCS flag. The decision
record retains only policy version, action, matched rule class, and a stable reason
code; it never retains the path, application, or matched pattern. Absolute paths,
traversal, controls, malformed escapes, and oversized inputs fail closed. A separate
`exclusion-policy-v1` JSON Schema rejects unknown fields and limits rule count.
Policy history is bounded to 64 retained versions.

## Target-device receipt

All receipts below were run from protected `main` commit
`6f4e6b9e9548cc7549ed4c5ab8710ad58f899899` on 2026-08-25:

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin` |
| Full debug all-target/all-feature suite | Passed before target cleanup; 16 library, 5 cursor, 4 fault, 1 FSEvents, 7 migration, 2 policy-state, 1 privacy, 5 support, 26 vertical, 6 WAL, and 5 writer tests. The raw debug target was task-owned and removed after the disk-limit retry. |
| Full release all-target/all-feature suite | `/private/tmp/ghostrace-0069-release-all-v2.log`; SHA-256 `5f2ffee871878b5bdbd67c3bb3d88bc0bb71e0e04a08bc3a6550e660182729a7` |
| Enforced macOS sandbox and focused release tests | `/private/tmp/ghostrace-0069-sandbox-v2.log`; SHA-256 `a905f65b7bc74fc05e43207278a6a100eb4319cd10c3f5f41701e5cc77cd29b2` |
| Static and repository checks | `/private/tmp/ghostrace-0069-static-v2.log`; SHA-256 `147d9c298b5369eccee7857a99db290d39dd4faf8aeb1ec685f149f21e8d2de7` |

The full release suite exited zero. Its migration crash-injection test prints the
expected failed child processes while the parent recovery assertion passes. The
enforced lane passed the direct release network-denial canary and focused exclusion,
policy-property, and privacy tests. Static checks passed formatting, locked release
Clippy with `-D warnings`, fixture/identity/release-evidence/roadmap/reproducibility
checks, 38 Python tests, generated-index parity, ShellCheck, actionlint, and the
product network-surface scan.

## Limitations

- The local Docker daemon is unavailable. The checked-in Linux workflow remains the
  Docker enforcement authority; this device receipt uses enforced macOS
  `sandbox-exec` and does not claim a Docker result.
- The matcher is a bounded policy primitive, not a filesystem canonicalization or
  symlink/open-race implementation. Those controls remain in parent task 0014 and
  its dependent gates.
- No real path, account name, credential, browser content, or live event was used.
