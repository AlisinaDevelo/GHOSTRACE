---
id: 0075
title: Exercise storms, sleep, wake, detach, and restart
status: done
agent: test-engineer
model: human
release: M2
parent: 0017
depends_on: [0014, 0015, 0016]
change: pr-268
workstream: filesystem
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Build a controlled macOS integration corpus for high-rate and lifecycle conditions that threaten coverage and ordering.

## Acceptance criteria
- [x] Scenarios include bulk checkout, package install, rename storm, directory deletion, sleep and wake, logout, volume detach, process kill, and collector restart.
- [x] Every scenario has a ground-truth operation log, expected direct observations, permitted coalescing, required gaps, recovery expectation, and a resource bound.
- [x] Repeated fixture replays and native macOS runs publish omission, duplication, ordering, recovery, and resource distributions rather than a single pass result.

## Context
The filesystem collector must be evaluated against realistic burst and lifecycle behavior, not only unit callbacks.

## Notes
Implemented in PR #268 and merged to protected `main` at
`e6bd66389b903354d781763391ebcb04441dabf3`. Completion evidence is retained in
`docs/evidence/0075-storm-lifecycle-corpus.md`; sleep/wake, logout, and volume
detach remain explicit guarded no-go rows until an authorized interactive device
run exists. Issue closure alone is not evidence.
