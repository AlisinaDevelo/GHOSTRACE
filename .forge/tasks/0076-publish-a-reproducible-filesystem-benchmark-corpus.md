---
id: 0076
title: Publish a reproducible filesystem benchmark corpus
status: done
agent: performance-engineer
model: human
release: M2
parent: 0017
depends_on: [0075]
change: pr-270
workstream: filesystem
type: test
priority: p0
risks: [privacy]
platform: macos
---

## Goal
Version the workloads, machines, operating systems, settings, ground truth, and analysis needed to compare collector correctness and cost over time.

## Acceptance criteria
- [x] The corpus includes small, deep, wide, Unicode, case-variant, Git, build-output, and event-storm trees generated without user data.
- [x] Reports include latency percentiles, coverage classes, duplicate rate, gap rate, CPU, memory, energy, and disk growth.
- [x] Results name hardware and OS context and never claim cross-machine comparability without normalization.

## Context
A benchmark is useful only when its workload and limitations are reproducible.

## Notes
Implemented in PR #270 and merged to protected `main` at
`e1f6a14ef445f5068dd8c52c9188c3a0e4ad41a0`. Completion evidence is retained in
`docs/evidence/M2-fsevents-benchmark.md`; the public issue is not closed until
that evidence is merged and the post-merge device reproduction is recorded.
