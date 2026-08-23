---
id: 0072
title: Define startup history-done and invalid-cursor behavior
status: backlog
agent: macos-engineer
model: human
release: M2
parent: 0015
depends_on: [0065, 0070]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Distinguish historical replay completion, live delivery, unavailable history, and invalid resume positions during collector startup.

## Acceptance criteria
- [ ] HistoryDone changes collector state through a tested transition rather than becoming a user event.
- [ ] SinceNow, stale, future, zero, wrapped, and corrupted cursors have explicit permitted or refused behavior.
- [ ] Startup timeout and partial-history failures produce a gap and never report the collector as fully live.

## Context
Collector readiness is an evidence state, not merely successful stream creation.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
