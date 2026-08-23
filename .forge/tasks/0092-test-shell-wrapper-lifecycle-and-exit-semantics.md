---
id: 0092
title: Test shell wrapper lifecycle and exit semantics
status: backlog
agent: test-engineer
model: human
release: M4
parent: 0024
depends_on: [0091]
change: null
workstream: shell-git
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Preserve the wrapped program's status and terminal behavior while recording bounded start, completion, signal, exec, and abandonment evidence.

## Acceptance criteria
- [ ] Tests cover normal exit, signal, exec failure, shell built-in, pipeline, timeout, cancellation, terminal close, and wrapper crash.
- [ ] The wrapper returns the child status according to the documented shell contract.
- [ ] Incomplete executions become explicit terminal gaps and never receive a fabricated end time or success status.

## Context
An observability wrapper must not change command semantics or manufacture completion evidence.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
