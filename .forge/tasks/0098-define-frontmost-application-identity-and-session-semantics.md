---
id: 0098
title: Define frontmost application identity and session semantics
status: backlog
agent: macos-engineer
model: human
release: M4
parent: 0027
depends_on: [0006, 0007, 0010, 0012]
change: null
workstream: frontmost
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Normalize activation observations into bounded signed-bundle identity, session transitions, and explicit unknowns without retaining titles or document context.

## Acceptance criteria
- [ ] The schema distinguishes bundle identifier, executable signing identity, launch instance, activation, deactivation, termination, and unknown app.
- [ ] Window titles, document names, URLs, accessibility data, menu state, and screen contents are structurally absent.
- [ ] Unsigned, translocated, helper, command-line, and rapidly switching applications have tested outcomes.

## Context
NSWorkspace activation is contextual evidence and not proof that an application caused a filesystem change.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
