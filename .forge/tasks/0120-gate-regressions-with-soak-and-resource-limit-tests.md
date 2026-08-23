---
id: 0120
title: Gate regressions with soak and resource-limit tests
status: backlog
agent: performance-engineer
model: human
release: M6
parent: 0039
depends_on: [0119]
change: null
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Catch sustained leaks, checkpoint starvation, index drift, unbounded queues, pathological queries, and energy regressions before release.

## Acceptance criteria
- [ ] Bounded CI tests protect fast regressions and scheduled macOS soaks exercise multi-hour and large-journal behavior.
- [ ] The scheduled gate fails any CPU, RSS, ingest, acknowledgement, query, export, soak-growth, or WAL-recovery threshold defined in task 0039, and reports the breached value and measurement uncertainty.
- [ ] Failures retain workload, environment, statistics, and minimized evidence without user data.
- [ ] A release cannot waive a breached hard safety bound; performance waivers require an owner, expiry, and published impact.

## Context
Short microbenchmarks do not reveal long-reader, WAL, leak, thermal, or queue accumulation failures.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
