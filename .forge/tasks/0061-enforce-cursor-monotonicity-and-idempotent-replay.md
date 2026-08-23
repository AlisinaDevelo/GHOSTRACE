---
id: 0061
title: Enforce cursor monotonicity and idempotent replay
status: backlog
agent: concurrency-specialist
model: human
release: M1
parent: 0010
depends_on: [0059, 0060]
change: null
workstream: storage
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Prevent duplicate, regressed, skipped, or cross-source cursors from corrupting recovery and coverage claims.

## Acceptance criteria
- [ ] Cursor types define source identity, comparison, reset, wrap, and invalidation semantics.
- [ ] Duplicate deliveries are idempotent while divergent events at the same cursor fail closed.
- [ ] Property tests cover reordering, replay, crash, source replacement, and policy-version changes.

## Context
Cursor state is part of the evidence boundary even when it is not encrypted payload data.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
