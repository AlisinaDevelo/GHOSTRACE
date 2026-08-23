---
id: 0039
title: Establish performance and resource benchmark gates
status: backlog
agent: maintainer
model: human
release: M6
depends_on: [0013, 0018, 0035, 0037]
change: null
---

## Goal
Turn daily-use resource expectations into reproducible budgets that catch regressions before release.

## Acceptance criteria
- [ ] Versioned budgets and reports cover idle CPU and resident memory.
- [ ] Reports cover ingest throughput and durable latency.
- [ ] Reports cover WAL growth, query speed, and export speed.

## Context
Measurements must name hardware, macOS version, journal size, collector mix, workload, and warm-up method so comparisons remain meaningful.

## Notes
No implementation notes yet.
