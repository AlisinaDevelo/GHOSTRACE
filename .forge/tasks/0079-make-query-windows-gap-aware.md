---
id: 0079
title: Make query windows gap-aware
status: done
agent: implementation-engineer
model: human
release: M3
parent: 0018
depends_on: [0071, 0077, 0078]
change: pr-278
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Return coverage summaries and intersecting gaps with every bounded query so consumers cannot mistake sparse results for complete history.

## Acceptance criteria
- [x] Coverage reports distinguish no events observed, source disabled, policy denied, source gap, retention deletion, and unknown history.
- [x] Window and source filters cannot hide a relevant gap without an explicit opt-out marker in the response.
- [x] Golden tests cover nested, adjacent, open-ended, and cross-source gap intervals.

## Context
An empty result is ambiguous unless the product explains what sources actually covered that interval.

## Notes
Implemented by PR #278 and evidenced by
[`docs/evidence/0079-gap-aware-query-windows.md`](../../docs/evidence/0079-gap-aware-query-windows.md).
Completion requires the acceptance evidence above; issue closure alone is not evidence.
