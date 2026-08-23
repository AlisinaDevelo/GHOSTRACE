---
id: 0064
title: Implement selected-root consent and lifecycle receipts
status: backlog
agent: privacy-engineer
model: human
release: M2
parent: 0013
depends_on: [0007, 0012]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Require an explicit, inspectable grant for every watched root and record enable, pause, scope-change, and disable transitions.

## Acceptance criteria
- [ ] The user sees canonical root identity, exclusions, retained fields, and known coverage limits before enabling capture.
- [ ] A receipt binds the root scope to an immutable policy version without storing path content in diagnostics.
- [ ] Revocation stops observation and produces a bounded terminal status before the command returns.

## Context
Filesystem permission and product consent are separate gates; both must be visible.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
