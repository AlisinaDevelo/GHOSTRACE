---
id: 0032
title: Implement browser bookmark event and snapshot collector
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0007, 0030, 0031]
change: null
---

## Goal
Add explicitly permitted bookmark history with sanitized identifiers and replay-safe snapshot behavior.

## Acceptance criteria
- [ ] Bookmark access requires explicit permission.
- [ ] Create, update, delete, and snapshot behavior is supported.
- [ ] URLs are sanitized and titles are omitted by default.
- [ ] Replay and deduplication are tested.

## Context
Bookmark collection is distinct from navigation collection and must have its own permission and policy decision. Snapshots must not create duplicate history on restart.

## Notes
No implementation notes yet.
