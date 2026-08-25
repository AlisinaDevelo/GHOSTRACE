---
id: 0066
title: Track volume identity and mount transitions
status: done
agent: macos-engineer
model: human
release: M2
parent: 0015
depends_on: [0013, 0014]
change: pr-250
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Bind cursors and selected roots to stable volume evidence so detach, remount, clone, restore, and replacement cannot masquerade as continuous history.

## Acceptance criteria
- [x] Volume identity includes documented stable fields and explicitly excludes mutable display names as sole identity.
- [x] Mount, unmount, device replacement, APFS snapshot restore, and path reuse produce tested discontinuity outcomes.
- [x] A cursor from one volume is never applied to another even when the path string matches.

## Context
FSEvents ID semantics differ between per-host and per-device streams. Cursor evidence must therefore bind the selected stream mode to its device and volume identity; path continuity alone is not evidence continuity.

## Notes
Implemented as a path-free `VolumeIdentity`, explicit `VolumeObservation` /
`VolumeTransition` contract, and cursor binding that requires matching volume
and FSEvents stream mode. The selected-root collector records device/filesystem
identity and scopes path digests to it. Durable cursor persistence remains task
0070/0015 scope; no live cursor is enabled by this task.
