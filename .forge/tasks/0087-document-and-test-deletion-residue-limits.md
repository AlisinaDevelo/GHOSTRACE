---
id: 0087
title: Document and test deletion residue limits
status: backlog
agent: security-auditor
model: human
release: M3
parent: 0021
depends_on: [0086]
change: null
workstream: explain-export
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Make database, WAL, free-page, index, backup, filesystem snapshot, export, and key-destruction residue explicit rather than promising perfect erasure.

## Acceptance criteria
- [ ] Deletion modes state their guarantees, costs, unsupported media behavior, and interaction with SQLite secure_delete and VACUUM.
- [ ] Tests inspect database, WAL, SHM, temporary, FTS or archive shadow structures, and backups for sentinels where technically meaningful.
- [ ] The UI and CLI distinguish logical deletion, compaction, cryptographic erasure, and external-copy responsibility.

## Context
SQLite documents that secure_delete has limits, including virtual-table shadow storage, and filesystems may retain snapshots.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
