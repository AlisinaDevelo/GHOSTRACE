---
id: 0063
title: Build a memory-safe FSEvents stream lifecycle adapter
status: backlog
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
- [ ] Callback pointers, context ownership, panic containment, and shutdown ordering are documented and tested.
- [ ] Start, stop, restart, and partial-initialization failures release every native resource exactly once.
- [ ] Thread and run-loop assumptions are asserted in macOS integration tests and sanitizer runs.

## Context
The first live collector crosses an unsafe FFI boundary and must isolate that risk from normalization and persistence.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
