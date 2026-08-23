---
id: 0099
title: Test frontmost sleep, wake, and privacy transitions
status: backlog
agent: test-engineer
model: human
release: M4
parent: 0028
depends_on: [0018, 0098]
change: null
workstream: frontmost
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Define coverage across startup, login, fast switching, lock, sleep, wake, Mission Control, app termination, and observer restart.

## Acceptance criteria
- [ ] A state-machine corpus identifies which transitions are direct notifications, inferred closures, or gaps.
- [ ] Private applications and user exclusions are filtered before persistence.
- [ ] Missed notifications and observer downtime never extend a prior app session as if coverage were continuous.

## Context
Application activity intervals must not be fabricated across observer or session gaps.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
