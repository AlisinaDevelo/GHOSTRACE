---
id: 0075
title: Exercise storms, sleep, wake, detach, and restart
status: backlog
agent: test-engineer
model: human
release: M2
parent: 0017
depends_on: [0014, 0015, 0016]
change: null
workstream: filesystem
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Build a controlled macOS integration corpus for high-rate and lifecycle conditions that threaten coverage and ordering.

## Acceptance criteria
- [ ] Scenarios include bulk checkout, package install, rename storm, directory deletion, sleep and wake, logout, volume detach, and process kill.
- [ ] Every scenario has a ground-truth operation log, expected direct observations, permitted coalescing, and required gaps.
- [ ] Repeated runs publish omission, duplication, ordering, recovery, and resource distributions rather than a single pass result.

## Context
The filesystem collector must be evaluated against realistic burst and lifecycle behavior, not only unit callbacks.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
