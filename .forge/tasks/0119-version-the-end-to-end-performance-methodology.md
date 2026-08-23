---
id: 0119
title: Version the end-to-end performance methodology
status: backlog
agent: performance-engineer
model: human
release: M6
parent: 0039
depends_on: [0017, 0018, 0035]
change: null
workstream: release-scale
type: test
priority: p0
risks: [privacy]
platform: macos
---

## Goal
Define representative workloads, hardware classes, OS context, warmup, sampling, statistical reporting, and pass or investigate thresholds for every public path.

## Acceptance criteria
- [ ] Workloads cover idle, sustained ingest, storms, queries, explanations, retention, verification, export, UI, and restart recovery.
- [ ] Reports include latency distributions, throughput, CPU, memory, energy, database and WAL growth, amplification, and queue pressure.
- [ ] The harness directly measures every numeric threshold in task 0039 on the minimum supported Mac, with fixed seeds, corpus digests, warm-up, repetitions, and documented confidence intervals.
- [ ] Baseline updates require a reviewed reason and retain comparable historical results.

## Context
Resource targets are meaningful only when the workload and measurement method are fixed.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
