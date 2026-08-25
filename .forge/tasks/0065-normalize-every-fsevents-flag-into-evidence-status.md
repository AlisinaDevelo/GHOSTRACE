---
id: 0065
title: Normalize every FSEvents flag into evidence status
status: done
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
- [x] Every documented Apple flag has a canonical representation or an explicit unsupported refusal.
- [x] Unknown future flag bits are retained as bounded numeric evidence and lower completeness rather than being ignored.
- [x] Golden callback batches cover compound and contradictory flag combinations.

## Context
Apple exposes coverage-changing flags such as UserDropped, KernelDropped, EventIdsWrapped, and RootChanged.

## Notes
Implemented as `fsevents-normalized-v1`. All 23 documented Apple event bits map to
typed canonical flags; dropped and wrapped coverage produces explicit rescan status,
unknown future bits are retained and lower completeness, and contradictory item
kind or mount-state combinations are refused without discarding the raw word. See
`docs/evidence/0065-fsevents-flag-normalization.md` for the flag table and device
receipts.
