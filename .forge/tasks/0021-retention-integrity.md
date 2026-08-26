---
id: 0021
title: Add retention, deletion, and integrity-check commands
status: done
agent: maintainer
model: human
release: M3
depends_on: [0009, 0018, 0020, 0086, 0087]
change: pr-306
workstream: explain-export
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Give users precise control over journal lifetime and provide safe local checks and recovery guidance for storage health.

## Acceptance criteria
- [x] The default retention period is documented.
- [x] Dry-run reports exact affected counts.
- [x] Deletion is scoped and transactional.
- [x] Integrity checks and recovery guidance work.

## Context
Retention is a privacy control, not only storage management. Destructive commands must resolve exact scope before changing data and leave auditable results.

## Notes
Implemented in PR #306 and reproduced on merged protected main. Logical deletion
does not compact SQLite, destroy keys, or remove external copies; integrity
failure guidance is stop-and-recover-on-a-verified-copy.
