---
id: 0163
title: Bring the native FSEvents benchmark within its device bound
status: done
agent: performance-engineer
model: human
release: M6
parent: 0039
depends_on: [0017]
change: pr-372
workstream: release-scale
type: test
priority: p1
risks: [privacy]
platform: macos
---

## Goal
Find why the native filesystem benchmark exceeds its 30-second per-scenario bound on the reference M1 device and restore a passing, unmodified bound.

## Acceptance criteria
- [x] A profile identifies where the scenario time goes (event delivery latency, writer commits, journal I/O, or harness waits).
- [x] The fix keeps the 30-second bound, loss accounting, and durability guarantees unchanged.
- [x] The offline network-denial lane passes end to end on the reference device, including the native benchmark.

## Context
Recent evidence records show the scenario taking 88 to 174 seconds and exiting 101, so every device verification reports an explicit resource no-go.

## Notes
Implemented in PR #372 and squash-merged to protected `main` at
`9dad5e04eb490287c6465455f06e9c75f15d10d3`. Verified on merged `main`; see
[`docs/evidence/0163-fsevents-benchmark-bound.md`](../../docs/evidence/0163-fsevents-benchmark-bound.md)
for command-level device receipts and limitations.
