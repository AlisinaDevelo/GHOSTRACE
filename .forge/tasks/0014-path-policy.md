---
id: 0014
title: Enforce root canonicalization, symlink, and exclusion rules
status: done
agent: maintainer
model: human
release: M2
depends_on: [0007, 0013, 0067, 0068, 0069]
change: pr-248-15468851538a1f070bc200a490163f99753a64e9
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Prevent filesystem observation from escaping selected roots or bypassing exclusions through path ambiguity and symbolic links.

## Acceptance criteria
- [x] Outside-root paths and symlink escapes are rejected.
- [x] Configured exclusions are hard enforced.
- [x] Blocked counts are visible without retaining blocked sensitive paths.
- [x] Unicode and malformed-path tests pass.

## Context
Path handling is a privacy boundary. Normalization, comparison, display, and diagnostics must remain safe for attacker-shaped names.

## Notes
The selected-root collector now resolves canonical filesystem identity before
policy admission, refuses lexical and descriptor escapes, and evaluates the
versioned deny-by-default policy before constructing a digest or event. Root,
symlink, malformed-path, Unicode, exclusion, and blocked-summary behavior is
covered by the retained test and device evidence in
`docs/evidence/0014-path-policy.md`.
