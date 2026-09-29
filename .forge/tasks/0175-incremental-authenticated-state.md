---
id: 0175
title: Make per-write authenticated-state verification incremental
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0088]
change: null
workstream: storage
type: feature
priority: p2
risks: [security]
platform: any
---

## Goal
Keep each journal write's authenticated-state check fast as the journal grows, without weakening tamper detection.

## Acceptance criteria
- [ ] A write verifies and advances the anchor from the previous anchor and the rows it adds, not by recomputing digests over every stored event.
- [ ] The full recomputation remains available and runs in `authenticated-check`, at startup after an unclean shutdown, and when another process's commit is detected.
- [ ] Write-lock hold time stays within a documented bound at 100,000 events on the reference device, and two concurrent writers never time out under the default busy timeout.

## Context
Every write recomputes the canonical snapshot over all events while holding the write lock (since the lock fix in #402), so a writer's lock hold time grows with the journal and a second GHOSTRACE process can wait past the default 250 ms busy timeout. The live CLI waits up to 10 s as a stopgap.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
