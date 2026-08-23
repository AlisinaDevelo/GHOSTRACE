---
id: 0066
title: Track volume identity and mount transitions
status: backlog
agent: macos-engineer
model: human
release: M2
parent: 0015
depends_on: [0013, 0014]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Bind cursors and selected roots to stable volume evidence so detach, remount, clone, restore, and replacement cannot masquerade as continuous history.

## Acceptance criteria
- [ ] Volume identity includes documented stable fields and explicitly excludes mutable display names as sole identity.
- [ ] Mount, unmount, device replacement, APFS snapshot restore, and path reuse produce tested discontinuity outcomes.
- [ ] A cursor from one volume is never applied to another even when the path string matches.

## Context
FSEvents ID semantics differ between per-host and per-device streams. Cursor evidence must therefore bind the selected stream mode to its device and volume identity; path continuity alone is not evidence continuity.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
