---
id: 0011
title: Build fixture replay and crash-injection harness
status: backlog
agent: maintainer
model: human
release: M1
depends_on: [0006, 0010, 0062]
change: null
workstream: storage
type: test
priority: p0
risks: [security]
platform: any
---

## Goal
Build repeatable evidence that normalized events, acknowledgements, and loss reporting remain correct across replay and process failure.

## Acceptance criteria
- [ ] Deterministic multi-source fixtures replay identically.
- [ ] Injected crashes create no false acknowledgements.
- [ ] Any loss becomes an explicit gap.

## Context
The corpus should exercise filesystem, frontmost-application, shell, Git, and browser-shaped inputs without enabling live collectors.

## Notes
The checked-in multi-source causal fixture replays deterministically and carries an explicit gap. Crash injection and false-acknowledgement tests remain open.
