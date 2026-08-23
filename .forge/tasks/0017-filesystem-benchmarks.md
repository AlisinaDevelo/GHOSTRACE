---
id: 0017
title: Publish filesystem correctness and latency benchmarks
status: backlog
agent: maintainer
model: human
release: M2
depends_on: [0013, 0014, 0015, 0016, 0075, 0076]
change: null
workstream: filesystem
type: test
priority: p0
risks: []
platform: macos
---

## Goal
Publish reproducible measurements of what the filesystem collector observes, misses, duplicates, delays, and recovers.

## Acceptance criteria
- [ ] Repeated runs report detection latency and duplicate events.
- [ ] Repeated runs report missing events and ordering behavior.
- [ ] Cursor recovery and explicit gap behavior are measured.
- [ ] Results are evaluated against documented thresholds.

## Context
The benchmark is a correctness contract as well as a performance report. Hardware, operating-system version, workload, and limitations must be recorded.

## Notes
No implementation notes yet.
