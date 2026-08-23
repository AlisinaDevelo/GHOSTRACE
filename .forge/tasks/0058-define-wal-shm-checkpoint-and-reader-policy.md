---
id: 0058
title: Define WAL, SHM, checkpoint, and reader policy
status: backlog
agent: database-expert
model: human
release: M1
parent: 0009
depends_on: [0057]
change: null
workstream: storage
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Bound WAL growth and sidecar exposure while preserving one-writer and read-only query semantics.

## Acceptance criteria
- [ ] Checkpoint thresholds, busy handling, long-reader limits, and shutdown behavior are explicit and measured.
- [ ] WAL and SHM files are verified after creation and never copied independently as a valid backup.
- [ ] Checkpoint starvation and abrupt-termination tests prove bounded disk usage or emit an actionable refusal.

## Context
SQLite WAL permits concurrent readers but one writer, can grow under long readers, and is not suitable for network filesystems.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
