---
id: 0065
title: Normalize every FSEvents flag into evidence status
status: backlog
agent: macos-engineer
model: human
release: M2
parent: 0013
depends_on: [0006, 0063]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Map item, history, drop, wrap, root-change, mount, unmount, clone, ownership, and metadata flags without discarding source uncertainty.

## Acceptance criteria
- [ ] Every documented Apple flag has a canonical representation or an explicit unsupported refusal.
- [ ] Unknown future flag bits are retained as bounded numeric evidence and lower completeness rather than being ignored.
- [ ] Golden callback batches cover compound and contradictory flag combinations.

## Context
Apple exposes coverage-changing flags such as UserDropped, KernelDropped, EventIdsWrapped, and RootChanged.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
