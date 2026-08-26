---
id: 0087
title: Document and test deletion residue limits
status: done
agent: security-auditor
model: human
release: M3
parent: 0021
depends_on: [0086]
change: pr-304
workstream: explain-export
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Make database, WAL, free-page, index, backup, filesystem snapshot, export, and key-destruction residue explicit rather than promising perfect erasure.

## Acceptance criteria
- [x] Deletion modes state their guarantees, costs, unsupported media behavior, and interaction with SQLite secure_delete and VACUUM.
- [x] Tests inspect database, WAL, SHM, temporary, FTS or archive shadow structures, and backups for sentinels where technically meaningful.
- [x] The UI and CLI distinguish logical deletion, compaction, cryptographic erasure, and external-copy responsibility.

## Context
SQLite documents that secure_delete has limits, including virtual-table shadow storage, and filesystems may retain snapshots.

## Notes
Implemented in PR #304 and reproduced on merged protected main. The read-only residue report documents limits; deletion, compaction, and key-destruction operations remain owned by parent task 0021.
