# Task 0014 evidence: root and exclusion policy enforcement

Status: complete on protected `main` after PR #248 and its evidence readback.
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

## Verification

The final protected-main commands, exact commit, device details, log digests,
and hosted merge checks are recorded in the closed GitHub issue for this task.
The local evidence files are deliberately path-free and contain no callback
path, account data, credential, or capture key.
