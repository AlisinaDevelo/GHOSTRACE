---
id: 0052
title: Model consent as a revocable state machine
status: done
agent: privacy-engineer
model: human
release: M1
parent: 0007
depends_on: [0004, 0006]
change: null
workstream: privacy
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Represent grant, scope change, suspension, revocation, and deletion intent as explicit local state transitions with user-visible receipts.

## Acceptance criteria
- [x] Every transition records policy identity, version, scope digest, time, actor, and reason without sensitive content.
- [x] Revocation stops new retention before any asynchronous cleanup begins.
- [x] Crash and replay tests prove no transition can silently re-enable a collector.

## Context
Consent is ongoing control, not a one-time checkbox.

## Notes
Implemented in `c16baa19a55be1b64e1cace77ddd720f28c86ce4` and verified in
`docs/evidence/0052-consent-state-machine.md`. Completion evidence covers bounded
receipts, synchronous revocation, and fail-closed replay.
