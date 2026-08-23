---
id: 0021
title: Add retention, deletion, and integrity-check commands
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0009, 0018, 0020, 0086, 0087]
change: null
workstream: explain-export
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Give users precise control over journal lifetime and provide safe local checks and recovery guidance for storage health.

## Acceptance criteria
- [ ] The default retention period is documented.
- [ ] Dry-run reports exact affected counts.
- [ ] Deletion is scoped and transactional.
- [ ] Integrity checks and recovery guidance work.

## Context
Retention is a privacy control, not only storage management. Destructive commands must resolve exact scope before changing data and leave auditable results.

## Notes
No implementation notes yet.
