---
id: 0017
title: Publish filesystem correctness and latency benchmarks
status: done
agent: maintainer
model: human
release: M2
depends_on: [0013, 0014, 0015, 0016, 0075, 0076]
change: pr-295
workstream: filesystem
type: test
priority: p0
risks: []
platform: macos
---

## Goal
Publish reproducible measurements of what the filesystem collector observes, misses, duplicates, delays, and recovers.

## Acceptance criteria
- [x] Repeated runs report detection latency and duplicate events.
- [x] Repeated runs report missing events and ordering behavior.
- [x] Cursor recovery and explicit gap behavior are measured.
- [x] Results are evaluated against documented thresholds.

## Context
The benchmark is a correctness contract as well as a performance report. Hardware, operating-system version, workload, and limitations must be recorded.

## Notes
The versioned corpus and native-safe workload are implemented in PRs #268 and #270. The consolidated protected-main benchmark receipt is retained in [docs/evidence/0017-filesystem-benchmarks.md](../../docs/evidence/0017-filesystem-benchmarks.md). Completion requires the acceptance evidence above; issue closure alone is not evidence.
