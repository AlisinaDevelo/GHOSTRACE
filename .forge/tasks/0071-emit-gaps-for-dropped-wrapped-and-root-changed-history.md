---
id: 0071
title: Emit gaps for dropped, wrapped, and root-changed history
status: done
agent: macos-engineer
model: human
release: M2
parent: 0015
depends_on: [0065, 0070]
change: pr-254-0caed8b3
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Translate coverage-changing FSEvents flags into bounded first-class gap records and a safe next recovery action.

## Acceptance criteria
- [x] UserDropped, KernelDropped, EventIdsWrapped, RootChanged, and MustScanSubDirs each have distinct reason codes.
- [x] The selected-root stream enables WatchRoot, and a root replacement test proves that RootChanged cannot be silently missed.
- [x] The gap records the affected source, volume, roots, cursor range when knowable, and remediation without claiming complete enumeration.
- [x] Recovery never resumes with a continuous-coverage claim until a documented reconciliation completes.

## Context
Dropped notifications require a rescan or explicit gap; they cannot be treated as ordinary events. Apple emits RootChanged only for streams created with WatchRoot, so that prerequisite is part of the evidence contract.

## Notes
Implemented in the normalized flag contract and selected-root collector. A
source-loss gap sets `recovery_required`; the later reconciliation and durable
restart flow remain task 0015/0072 work. Completion requires the acceptance
evidence above; issue closure alone is not evidence.
