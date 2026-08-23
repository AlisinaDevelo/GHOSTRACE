---
id: 0127
title: Build guided backup, restore, and upgrade recovery
status: backlog
agent: database-expert
model: human
release: M7
parent: 0125
depends_on: [0040]
change: null
workstream: operations
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Provide verified local backup and restoration of journal, schema, keys, policy, and receipts with dry-run, rollback, and explicit unrecoverable states.

## Acceptance criteria
- [ ] Backups use SQLite-safe snapshot semantics and bind manifests, key requirements, source version, and integrity checkpoints.
- [ ] Restore never overwrites the active journal without a verified backup, dry-run, user confirmation, and rollback copy.
- [ ] Cross-version, partial, corrupted, wrong-key, stale-policy, disk-full, and interrupted cases are exercised.

## Context
Copying a live WAL database file is not a valid backup strategy.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
