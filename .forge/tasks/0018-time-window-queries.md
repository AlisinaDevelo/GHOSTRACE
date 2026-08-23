---
id: 0018
title: Implement time-window queries and stable ordering
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0010, 0012]
change: null
---

## Goal
Provide a deterministic query foundation for investigations across sources, roots, event kinds, and imperfect clocks.

## Acceptance criteria
- [ ] Queries filter by time, source, root, and kind.
- [ ] Results order by observed time and then ingest sequence.
- [ ] Clock skew is handled explicitly.
- [ ] Policy-blocked data is never returned.

## Context
Ingest sequence is the durable journal order, while source timestamps retain when an event was observed. Both are needed to explain clock ambiguity honestly.

## Notes
No implementation notes yet.
