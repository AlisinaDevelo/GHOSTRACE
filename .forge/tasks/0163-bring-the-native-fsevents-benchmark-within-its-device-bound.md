---
id: 0163
title: Bring the native FSEvents benchmark within its device bound
status: backlog
agent: performance-engineer
model: human
release: M6
parent: 0039
depends_on: [0017]
change: null
workstream: release-scale
type: test
priority: p1
risks: [privacy]
platform: macos
---

## Goal
Find why the native filesystem benchmark exceeds its 30-second per-scenario bound on the reference M1 device and restore a passing, unmodified bound.

## Acceptance criteria
- [ ] A profile identifies where the scenario time goes (event delivery latency, writer commits, journal I/O, or harness waits).
- [ ] The fix keeps the 30-second bound, loss accounting, and durability guarantees unchanged.
- [ ] The offline network-denial lane passes end to end on the reference device, including the native benchmark.

## Context
Recent evidence records show the scenario taking 88 to 174 seconds and exiting 101, so every device verification reports an explicit resource no-go.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
