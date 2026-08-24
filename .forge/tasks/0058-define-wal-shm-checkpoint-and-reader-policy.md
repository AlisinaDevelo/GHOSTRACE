---
id: 0058
title: Define WAL, SHM, checkpoint, and reader policy
status: done
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
- [x] Checkpoint thresholds, busy handling, long-reader limits, and shutdown behavior are explicit and measured.
- [x] WAL and SHM files are verified after creation and never copied independently as a valid backup.
- [x] Checkpoint starvation and abrupt-termination tests prove bounded disk usage or emit an actionable refusal.

## Context
SQLite WAL permits concurrent readers but one writer, can grow under long readers, and is not suitable for network filesystems.

## Notes
Implemented in PR #198 and merged to protected `main` at
`c9fc5bc664e105b7a002c235f6ecdab3a3d05485`; the post-merge evidence receipt was
reviewed and merged in PR #199 at
`8fe25394554df29dd1f2e32a752e20841698c98e`. The file-backed Journal applies an
explicit WAL policy, reports passive/truncate checkpoint frames and sidecar bytes,
bounds read-only snapshot lifetimes, copies only a checkpointed database for
backups, and recovers after an abrupt child exit. Source and merged-main device
pipes are retained in `docs/evidence/0058-wal-policy.md`.
