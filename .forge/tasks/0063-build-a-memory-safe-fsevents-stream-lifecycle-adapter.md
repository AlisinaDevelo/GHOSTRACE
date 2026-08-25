---
id: 0063
title: Build a memory-safe FSEvents stream lifecycle adapter
status: done
agent: macos-engineer
model: human
release: M2
parent: 0013
depends_on: [0010, 0012]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Wrap FSEventStream creation, scheduling, callbacks, flush, stop, invalidate, and release behind a bounded Rust ownership contract.

## Acceptance criteria
- [x] Callback pointers, context ownership, panic containment, and shutdown ordering are documented and tested.
- [x] Start, stop, restart, and partial-initialization failures release every native resource exactly once.
- [x] Thread and run-loop assumptions are asserted in macOS integration tests and sanitizer runs.

## Context
The first live collector crosses an unsafe FFI boundary and must isolate that risk from normalization and persistence.

## Notes
Implemented in `src/fsevents.rs` with the macOS integration test in
`tests/fsevents_lifecycle.rs`, lifecycle-model failure tests, and ADR 0004. The
device and protected-main receipts are retained in
`docs/evidence/0063-fsevents-stream-lifecycle.md`. The adapter is not the live
collector: root consent/canonicalization, flag normalization, cursor persistence,
backpressure, and journal integration remain later gates.
