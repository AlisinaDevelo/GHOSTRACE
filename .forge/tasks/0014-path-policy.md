---
id: 0014
title: Enforce root canonicalization, symlink, and exclusion rules
status: backlog
agent: maintainer
model: human
release: M2
depends_on: [0007, 0013, 0067, 0068, 0069]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Prevent filesystem observation from escaping selected roots or bypassing exclusions through path ambiguity and symbolic links.

## Acceptance criteria
- [ ] Outside-root paths and symlink escapes are rejected.
- [ ] Configured exclusions are hard enforced.
- [ ] Blocked counts are visible without retaining blocked sensitive paths.
- [ ] Unicode and malformed-path tests pass.

## Context
Path handling is a privacy boundary. Normalization, comparison, display, and diagnostics must remain safe for attacker-shaped names.

## Notes
No implementation notes yet.
