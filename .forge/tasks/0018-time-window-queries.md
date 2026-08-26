---
id: 0018
title: Implement time-window queries and stable ordering
status: done
agent: maintainer
model: human
release: M3
depends_on: [0010, 0012, 0077, 0078, 0079]
change: pr-296
workstream: explain-export
type: feature
priority: p1
risks: []
platform: any
---

## Goal
Provide a deterministic query foundation for investigations across sources, roots, event kinds, and imperfect clocks.

## Acceptance criteria
- [x] Queries filter by time, source, root, and kind.
- [x] Results order by observed time and then ingest sequence.
- [x] Clock skew is handled explicitly.
- [x] Policy-blocked data is never returned.

## Context
Ingest sequence is the durable journal order, while source timestamps retain when an event was observed. Both are needed to explain clock ambiguity honestly.

## Notes
Implemented in PR #296 and merged to protected `main` at
`403e1acd00948a8a9619e4ee671fdf2a19d23914`. The retained acceptance record is
`docs/evidence/0018-time-window-queries.md`; issue closure follows the evidence
merge and independent live-state verification.
