---
id: 0068
title: Harden symlink, hard-link, and open-race containment
status: done
agent: security-auditor
model: human
release: M2
parent: 0014
depends_on: [0013, 0057]
change: pr-246-a072248bac44f5444de19a00c9e626d2c4e63f21
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Prevent a selected root or event path from escaping scope through link replacement, aliasing, hard links, or validation-to-use races.

## Acceptance criteria
- [x] Root selection and later opens use descriptor-based or equivalent no-follow containment checks.
- [x] Hard-link and symlink events preserve source facts without reading target content outside scope.
- [x] Adversarial race tests mutate every path component and prove denial or explicit unknown coverage.

## Context
Path validation is not authorization when an attacker can change the namespace between checks.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.

## Completion

Merged implementation: PR #246 at `a072248bac44f5444de19a00c9e626d2c4e63f21`.
The protected-main device receipts and explicit no-go limits are retained in
`docs/evidence/0068-contained-open.md`.
