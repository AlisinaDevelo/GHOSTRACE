---
id: 0015
title: Persist FSEvents cursors and recover after restart
status: backlog
agent: maintainer
model: human
release: M2
depends_on: [0010, 0013, 0066, 0070, 0071, 0072]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Resume filesystem observation safely after restart while exposing any interval the source can no longer reconstruct.

## Acceptance criteria
- [ ] Each cursor and its events commit atomically.
- [ ] Restart resumes from the committed cursor.
- [ ] Invalid, wrapped, or dropped history emits a gap.
- [ ] The journal never claims completeness across an uncovered interval.

## Context
Cursor recovery is part of the journal's evidence quality. A durable gap is preferable to silently filling or hiding missing history.

## Notes
No implementation notes yet.
