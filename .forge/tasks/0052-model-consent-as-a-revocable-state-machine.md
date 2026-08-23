---
id: 0052
title: Model consent as a revocable state machine
status: ready
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
- [ ] Every transition records policy identity, version, scope digest, time, actor, and reason without sensitive content.
- [ ] Revocation stops new retention before any asynchronous cleanup begins.
- [ ] Crash and replay tests prove no transition can silently re-enable a collector.

## Context
Consent is ongoing control, not a one-time checkbox.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
