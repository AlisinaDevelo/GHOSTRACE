---
id: 0162
title: Close interrupted shell runs with explicit gaps
status: backlog
agent: test-engineer
model: human
release: M4
parent: 0024
depends_on: [0024]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Make a wrapped run that ends without an observed child status visible as an incomplete execution instead of a dangling `shell_started`.

## Acceptance criteria
- [ ] SIGTERM and SIGHUP delivered to the wrapper are forwarded to the child, and the wrapper records a terminal gap when it cannot observe the child's status.
- [ ] Query coverage and explanations report a `shell_started` without a terminal event as an incomplete run with no end time or success status.
- [ ] Tests cover terminal close, wrapper termination, and journal recovery after a wrapper crash.

## Context
The wrapper ignores SIGINT and SIGQUIT while it waits, but a terminal close or kill of the wrapper leaves only the start event.

## Notes
Follow-up to 0024. Completion requires the acceptance evidence above; issue closure alone is not evidence.
