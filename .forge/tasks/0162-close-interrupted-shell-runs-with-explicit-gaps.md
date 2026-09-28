---
id: 0162
title: Close interrupted shell runs with explicit gaps
status: done
agent: test-engineer
model: human
release: M4
parent: 0024
depends_on: [0024]
change: pr-351
workstream: shell-git
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Make a wrapped run that ends without an observed child status visible as an incomplete execution instead of a dangling `shell_started`.

## Acceptance criteria
- [x] SIGTERM and SIGHUP delivered to the wrapper are forwarded to the child, and the wrapper records a terminal gap when it cannot observe the child's status.
- [x] Query coverage and explanations report a `shell_started` without a terminal event as an incomplete run with no end time or success status.
- [x] Tests cover terminal close, wrapper termination, and journal recovery after a wrapper crash.

## Context
The wrapper ignores SIGINT and SIGQUIT while it waits, but a terminal close or kill of the wrapper leaves only the start event.

## Notes
Implemented in PR #351 and squash-merged to protected `main` at
`d4c0bb4d4a8175ea20e80f9cfa2e534ea8e0edc3`. Verified on merged `main` at `5a0bdbba25a97009d3bf520b90b83198e65da6ec`; see
[`docs/evidence/0162-interrupted-shell-runs.md`](../../docs/evidence/0162-interrupted-shell-runs.md)
for command-level device receipts and limitations.
