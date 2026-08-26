---
id: 0092
title: Test shell wrapper lifecycle and exit semantics
status: done
agent: test-engineer
model: human
release: M4
parent: 0024
depends_on: [0091]
change: pr-319
workstream: shell-git
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Preserve the wrapped program's status and terminal behavior while recording bounded start, completion, signal, exec, and abandonment evidence.

## Acceptance criteria
- [x] Tests cover normal exit, signal, exec failure, shell built-in, pipeline, timeout, cancellation, terminal close, and wrapper crash.
- [x] The wrapper returns the child status according to the documented shell contract.
- [x] Incomplete executions become explicit terminal gaps and never receive a fabricated end time or success status.

## Context
An observability wrapper must not change command semantics or manufacture completion evidence.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.

Implemented in PR #319 and squash-merged to protected `main` at
`fe0a8908a893ef47b6b45ab1cb869609a9c099b3`. Completion evidence is retained in
`docs/evidence/0092-shell-wrapper-lifecycle.md`.
