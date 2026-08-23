---
id: 0078
title: Model clock skew and deterministic total ordering
status: backlog
agent: architect
model: human
release: M3
parent: 0018
depends_on: [0010, 0012]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Keep source, observation, ingest, and monotonic timing evidence distinct and define one stable display order without implying causal order.

## Acceptance criteria
- [ ] Ordering keys and tie-breakers are versioned and deterministic across database and export implementations.
- [ ] Fixtures cover clock rollback, leap adjustments, sleep, equal timestamps, delayed batches, and missing source time.
- [ ] Explanations label temporal ambiguity whenever order depends on ingest sequence rather than source evidence.

## Context
Wall-clock order can be useful context but is not a causal proof.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
