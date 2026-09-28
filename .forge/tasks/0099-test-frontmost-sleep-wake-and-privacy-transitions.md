---
id: 0099
title: Test frontmost sleep, wake, and privacy transitions
status: done
agent: test-engineer
model: human
release: M4
parent: 0028
depends_on: [0018, 0098]
change: pr-357
workstream: frontmost
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Define coverage across startup, login, fast switching, lock, sleep, wake, Mission Control, app termination, and observer restart.

## Acceptance criteria
- [x] A state-machine corpus identifies which transitions are direct notifications, inferred closures, or gaps.
- [x] Private applications and user exclusions are filtered before persistence.
- [x] Missed notifications and observer downtime never extend a prior app session as if coverage were continuous.

## Context
Application activity intervals must not be fabricated across observer or session gaps.

## Notes
Implemented in PR #357 and squash-merged to protected `main` at
`3c5fdc595baf0b3d46a11f79db4d60f680c68893`. Verified on merged `main` at `5a0bdbba25a97009d3bf520b90b83198e65da6ec`; see
[`docs/evidence/0099-frontmost-coverage.md`](../../docs/evidence/0099-frontmost-coverage.md)
for command-level device receipts and limitations.
