---
id: 0039
title: Establish performance and resource benchmark gates
status: backlog
agent: maintainer
model: human
release: M6
depends_on: [0013, 0018, 0035, 0037, 0119, 0120]
change: null
workstream: release-scale
type: test
priority: p1
risks: []
platform: macos
---

## Goal
Turn daily-use resource expectations into reproducible budgets that catch regressions before release.

## Acceptance criteria
- [ ] On the minimum supported Mac named by task 0045 after warm-up, a 15-minute fixture-only idle run has median process CPU at or below 1.0%, p95 CPU at or below 2.0%, and resident memory at or below 200 MiB.
- [ ] On that Mac with 10 million retained records, the system sustains 1,000 normalized events per second for 10 minutes with p99 durable acknowledgement at or below 250 ms, zero unreported loss, and no configured queue-cap breach.
- [ ] On the same 10-million-record corpus, a fixed 24-hour timeline query completes at p95 at or below 500 ms and streaming JSONL export sustains at least 25,000 records per second.
- [ ] A 24-hour 100-event-per-second soak grows post-warm-up RSS by no more than 10%, and the WAL returns to at most 64 MiB within 60 seconds after the final long reader closes.
- [ ] Every report names hardware, macOS, power state, build, workload digest, journal shape, repetitions, and uncertainty; any proposed target change is reviewed in the release evidence register before the affected release gate runs.

## Context
These are initial planning thresholds, not current product claims. Task 0047 records their evidence state, and task 0119 freezes the reproducible method before task 0120 enforces them.

## Notes
No implementation notes yet.
