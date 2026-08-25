---
id: 0062
title: Build the storage crash and fault-injection matrix
status: done
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
- [x] Named fault points cover before, during, and after every durable state transition.
- [x] Each case asserts committed events, cursor state, stable fixture key generation, gaps, and retry behavior after restart.
- [x] The matrix runs bounded seeds in CI and preserves minimized failing schedules as regression fixtures.

## Context
Passing happy-path transactions does not prove recovery semantics under abrupt failure.

## Notes
Implemented in `src/fault.rs`, `src/journal.rs`, and `tests/fault_matrix.rs`.
Completion requires the retained device pipe and protected-main reproduction in
`docs/evidence/0062-storage-fault-matrix.md`; issue closure alone is not evidence.
