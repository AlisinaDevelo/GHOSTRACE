# Task 0014 evidence: root and exclusion policy enforcement

Status: complete on protected `main` at `15468851538a1f070bc200a490163f99753a64e9` after PR #248 and this evidence readback.
This record consolidates the root, Unicode, no-follow, exclusion, and blocked
summary contracts that make selected-root observation a privacy boundary.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Outside-root paths and symlink escapes are rejected | `SelectedRoot::contains_path` resolves the nearest existing component, rejects `..`, requires component-aware containment and matching device/inode identity, and refuses a replaced root. `SelectedRoot::open_contained` walks descriptors with `O_NOFOLLOW`, rejects symlink and hard-link aliases, and denies every component replacement. |
| Configured exclusions are hard enforced | `PolicyProfile::decide` checks source enablement, selected roots, and `excluded_roots` before the collector computes a path digest or constructs an event. The reproducible policy property matrix exercises excluded roots and verifies the `root_excluded` reason; the exclusion-policy suite covers deterministic deny/redact/summarize precedence, escaped patterns, nested paths, and malformed subjects. |
| Blocked counts are visible without retaining blocked sensitive paths | The collector increments bounded `blocked_events` for outside-root and denied-policy observations, emits `PolicyBlockedSummary` with a typed reason and count, and never includes the callback path. Policy decision records redact untrusted root text and the privacy tests assert that attacker-shaped values do not appear in serialized diagnostics. |
| Unicode and malformed-path tests pass | The selected-root scope suite compares composed/decomposed Unicode and case-only rename behavior according to the actual filesystem; malformed relative, parent traversal, lexical sibling, symlink, hard-link, and invalid policy inputs fail closed. |

## Implementation boundary

The collector retains only an opaque root ID, path class, operation, entry kind,
and root-scoped digest. It does not open or read callback paths. Exclusion
matching is versioned and evaluated only before persistence; existing evidence
retains the policy version used at admission. A blocked observation is counted
and summarized, never persisted as a full filesystem event.

## Scope limits

The current device receipt covers the Apple-silicon macOS environment named in
the verification table. It does not claim Intel, macOS 15, an external-volume
mount matrix, or a privileged attacker. The native collector remains explicit
no-go off macOS. Live FSEvents cursor recovery, volume-bound cursors, and storm
backpressure remain later M2 tasks.

## Target-device verification

The final replay ran on 2026-08-25 against the exact protected-main commit
above: MacBookPro17,1 (Apple M1 arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0,
Rust/Cargo 1.88.0, target `aarch64-apple-darwin`, Python 3.9.6.

| Lane | Result and retained receipt |
| --- | --- |
| Focused policy and containment suite | Pass: 10 collector unit tests, 4 exclusion-policy tests, 2 policy-state property tests, 3 consent tests, 2 selected-root open tests, and 2 Unicode/case scope tests; `/tmp/ghostrace-0014-postmerge-focused.log`; SHA-256 `9a41b69e40c879be3d523d2c3718078d32ca88a72998de8cee7afe6228e0e154` |
| Full reproducibility pipe | Pass: pinned inputs, fixture/schema/determinism checks, capture refusal, roadmap/evidence checks, 40 Python tests, Clippy, and all debug targets; `/tmp/ghostrace-0014-postmerge-repro.log`; SHA-256 `e5309a905d6c5c28b0d3aff62bdb9ed9d6663443d3b7950f4483ed4f957cfb8f` |
| Release all-target/all-feature tests | Pass: locked release suite; `/tmp/ghostrace-0014-postmerge-release.log`; SHA-256 `ba5607c209676e0f192fc79647557f612420794d65435ec5ebe39ab403658f68` |
| Rustdoc warnings denied | Pass with `RUSTDOCFLAGS='-D warnings'`; `/tmp/ghostrace-0014-postmerge-doc.log`; SHA-256 `2bf9c723c2f9032414ae6be299e9231763bbc94d260ccc9c440c1ee40d26cca3` |
| Offline/network-denial lane | Pass under macOS `sandbox-exec`: denial canary, privacy regression, and complete locked offline product suite; `/tmp/ghostrace-0014-postmerge-offline.log`; SHA-256 `dcb44b88dd4911bc08be1c220bfe376badd895e3c9f17fc89ef283f2030ffe68` |

The implementation PR's hosted checks also passed duplicate CI, Cargo policy,
advisory, dependency-review, roadmap, and network-denial jobs. The local
evidence files are deliberately path-free and contain no callback path,
account data, credential, or capture key.
