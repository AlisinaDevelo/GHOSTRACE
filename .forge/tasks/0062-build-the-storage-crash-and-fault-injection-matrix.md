---
id: 0062
title: Build the storage crash and fault-injection matrix
status: backlog
agent: test-engineer
model: human
release: M1
parent: 0011
depends_on: [0055, 0058, 0061]
change: null
workstream: storage
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Exercise deterministic failures across key access, SQLite calls, filesystem operations, process termination, and replay boundaries.

## Acceptance criteria
- [ ] Named fault points cover before, during, and after every durable state transition.
- [ ] Each case asserts committed events, cursor state, key generation, gaps, and retry behavior after restart.
- [ ] The matrix runs bounded seeds in CI and preserves minimized failing schedules as regression fixtures.

## Context
Passing happy-path transactions does not prove recovery semantics under abrupt failure.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
