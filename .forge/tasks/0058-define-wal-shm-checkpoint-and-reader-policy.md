---
id: 0058
title: Define WAL, SHM, checkpoint, and reader policy
status: review
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
Implementation is under review in the 0058 pull request. The file-backed Journal now
applies an explicit WAL policy, reports passive/truncate checkpoint frames and
sidecar bytes, bounds read-only snapshot lifetimes, and copies only a checkpointed
database for backups. The source-device pipe and the required merged-main rerun are
retained in `docs/evidence/0058-wal-policy.md`; the task remains `review` until the
merged-main receipt is appended.
