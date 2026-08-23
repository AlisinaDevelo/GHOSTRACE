---
id: 0004
title: Publish the threat model and data inventory
status: done
agent: maintainer
model: human
release: M0
depends_on: [0001, 0002]
change: null
workstream: foundation
type: docs
priority: p0
risks: [security]
platform: any
---

## Goal
Document what GHOSTRACE protects, where trust changes, how it can fail, and which risks remain outside its protection boundary.

## Acceptance criteria
- [x] The threat model documents assets, actors, and trust boundaries.
- [x] Attacker stories, severity calibration, and mitigations are included.
- [x] Residual same-user compromise risk is stated plainly.

## Context
The inventory must cover journal contents, keys, configuration, event ordering, exports, and user-visible privacy state.

## Notes
Verified in the M0 threat model and privacy data inventory. Roadmap mitigations remain labeled as future work rather than shipped protection.
