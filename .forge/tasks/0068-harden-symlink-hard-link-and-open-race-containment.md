---
id: 0068
title: Harden symlink, hard-link, and open-race containment
status: backlog
agent: security-auditor
model: human
release: M2
parent: 0014
depends_on: [0013, 0057]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Prevent a selected root or event path from escaping scope through link replacement, aliasing, hard links, or validation-to-use races.

## Acceptance criteria
- [ ] Root selection and later opens use descriptor-based or equivalent no-follow containment checks.
- [ ] Hard-link and symlink events preserve source facts without reading target content outside scope.
- [ ] Adversarial race tests mutate every path component and prove denial or explicit unknown coverage.

## Context
Path validation is not authorization when an attacker can change the namespace between checks.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
