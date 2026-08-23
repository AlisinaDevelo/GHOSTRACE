---
id: 0071
title: Emit gaps for dropped, wrapped, and root-changed history
status: backlog
agent: macos-engineer
model: human
release: M2
parent: 0015
depends_on: [0065, 0070]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Translate coverage-changing FSEvents flags into bounded first-class gap records and a safe next recovery action.

## Acceptance criteria
- [ ] UserDropped, KernelDropped, EventIdsWrapped, RootChanged, and MustScanSubDirs each have distinct reason codes.
- [ ] The selected-root stream enables WatchRoot, and a root replacement test proves that RootChanged cannot be silently missed.
- [ ] The gap records the affected source, volume, roots, cursor range when knowable, and remediation without claiming complete enumeration.
- [ ] Recovery never resumes with a continuous-coverage claim until a documented reconciliation completes.

## Context
Dropped notifications require a rescan or explicit gap; they cannot be treated as ordinary events. Apple emits RootChanged only for streams created with WatchRoot, so that prerequisite is part of the evidence contract.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
