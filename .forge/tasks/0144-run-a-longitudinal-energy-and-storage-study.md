---
id: 0144
title: Run a longitudinal energy and storage study
status: backlog
agent: performance-engineer
model: human
release: M9
parent: 0139
depends_on: [0120, 0140]
change: null
workstream: research
type: test
priority: p1
risks: [privacy]
platform: macos
---

## Goal
Measure multi-week synthetic operation, database growth, WAL behavior, retention, compaction, key rotation, query drift, energy use, and recovery cost across supported hardware classes.

## Acceptance criteria
- [ ] The protocol fixes workloads, duty cycles, OS and hardware context, thermal state, power source, retention policy, and measurement tools.
- [ ] Results report distributions, confidence or uncertainty, anomalies, maintenance events, and raw synthetic metrics.
- [ ] Hard resource-bound violations stop the study and produce actionable regression fixtures rather than damaging the host.

## Context
A continuously running local journal needs evidence beyond short benchmarks and one-day soak tests.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
