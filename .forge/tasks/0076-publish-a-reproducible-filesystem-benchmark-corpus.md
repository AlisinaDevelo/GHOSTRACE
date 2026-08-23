---
id: 0076
title: Publish a reproducible filesystem benchmark corpus
status: backlog
agent: performance-engineer
model: human
release: M2
parent: 0017
depends_on: [0075]
change: null
workstream: filesystem
type: test
priority: p0
risks: [privacy]
platform: macos
---

## Goal
Version the workloads, machines, operating systems, settings, ground truth, and analysis needed to compare collector correctness and cost over time.

## Acceptance criteria
- [ ] The corpus includes small, deep, wide, Unicode, case-variant, Git, build-output, and event-storm trees generated without user data.
- [ ] Reports include latency percentiles, coverage classes, duplicate rate, gap rate, CPU, memory, energy, and disk growth.
- [ ] Results name hardware and OS context and never claim cross-machine comparability without normalization.

## Context
A benchmark is useful only when its workload and limitations are reproducible.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
