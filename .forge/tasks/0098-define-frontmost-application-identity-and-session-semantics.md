---
id: 0098
title: Define frontmost application identity and session semantics
status: done
agent: macos-engineer
model: human
release: M4
parent: 0027
depends_on: [0006, 0007, 0010, 0012]
change: pr-350
workstream: frontmost
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Normalize activation observations into bounded signed-bundle identity, session transitions, and explicit unknowns without retaining titles or document context.

## Acceptance criteria
- [x] The schema distinguishes bundle identifier, executable signing identity, launch instance, activation, deactivation, termination, and unknown app.
- [x] Window titles, document names, URLs, accessibility data, menu state, and screen contents are structurally absent.
- [x] Unsigned, translocated, helper, command-line, and rapidly switching applications have tested outcomes.

## Context
NSWorkspace activation is contextual evidence and not proof that an application caused a filesystem change.

## Notes
Implemented in PR #350 and squash-merged to protected `main` at
`f5cab8c2a474b332f72eb1c8485736a4c843e1c0`. Verified on merged `main` at `4b84b424f76b441b1b328330301b22961634390c`; see
[`docs/evidence/0098-frontmost-identity.md`](../../docs/evidence/0098-frontmost-identity.md)
for command-level device receipts and limitations.
