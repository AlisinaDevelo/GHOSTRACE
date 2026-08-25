---
id: 0015
title: Persist FSEvents cursors and recover after restart
status: done
agent: maintainer
model: human
release: M2
depends_on: [0010, 0013, 0066, 0070, 0071, 0072]
change: pr-259-9675be84d9abf1c100db266c9bb5523d8ef487f7
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Resume filesystem observation safely after restart while exposing any interval the source can no longer reconstruct.

## Acceptance criteria
- [x] Each cursor and its events commit atomically.
- [x] Restart resumes from the committed cursor.
- [x] Invalid, wrapped, or dropped history emits a gap.
- [x] The journal never claims completeness across an uncovered interval.

## Context
Cursor recovery is part of the journal's evidence quality. A durable gap is preferable to silently filling or hiding missing history.

## Notes
Implemented in PR [#259](https://github.com/AlisinaDevelo/GHOSTRACE/pull/259),
merged to protected `main` at
`9675be84d9abf1c100db266c9bb5523d8ef487f7`. The implementation derives a
restart replay position from the committed cursor, records boundary/invalid/
wrapped recovery gaps durably, and invalidates the cursor in the same SQLite
transaction as the gap. The complete post-merge device receipt is retained in
`docs/evidence/0015-fsevents-recovery.md`.
