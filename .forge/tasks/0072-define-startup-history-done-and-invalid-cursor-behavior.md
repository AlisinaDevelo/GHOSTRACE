---
id: 0072
title: Define startup history-done and invalid-cursor behavior
status: done
agent: macos-engineer
model: human
release: M2
parent: 0015
depends_on: [0065, 0070]
change: pr-256-906dc362e13ec96b10be607e8961c066bbd8d9a0
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Distinguish historical replay completion, live delivery, unavailable history, and invalid resume positions during collector startup.

## Acceptance criteria
- [x] HistoryDone changes collector state through a tested transition rather than becoming a user event.
- [x] SinceNow, stale, future, zero, wrapped, and corrupted cursors have explicit permitted or refused behavior.
- [x] Startup timeout and partial-history failures produce a gap and never report the collector as fully live.

## Context
Collector readiness is an evidence state, not merely successful stream creation.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
