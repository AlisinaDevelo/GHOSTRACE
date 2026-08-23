---
id: 0044
title: Enforce an offline network-denial CI lane
status: ready
agent: security-auditor
model: human
release: M0
parent: 0005
depends_on: [0003, 0004]
change: null
workstream: privacy
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Prove that the fixture-only product path can build and execute its runtime tests without opening an outbound network surface.

## Acceptance criteria
- [ ] A pinned CI mechanism denies network access while running fixture, explanation, and export tests.
- [ ] A canary test attempts a connection and proves the denial mechanism is active rather than silently skipped.
- [ ] If hosted-runner constraints prevent enforcement, a checked-in decision record names the failed mechanism and an equivalent reproducible test.

## Context
Dependency download and runtime execution are separate phases; the denial applies to the exercised product path.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
