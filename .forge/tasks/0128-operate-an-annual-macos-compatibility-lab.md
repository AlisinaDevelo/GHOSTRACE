---
id: 0128
title: Operate an annual macOS compatibility lab
status: backlog
agent: macos-engineer
model: human
release: M7
parent: 0125
depends_on: [0040, 0045]
change: null
workstream: operations
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Continuously evaluate new macOS betas and releases against collectors, permissions, lifecycle, signing, notarization, Keychain, filesystem, and UI contracts.

## Acceptance criteria
- [ ] A versioned matrix runs synthetic smoke, recovery, permission, energy, and installer scenarios on each supported architecture.
- [ ] Behavior changes create tracked compatibility findings with affected surfaces, evidence, mitigation, and support decision.
- [ ] Support additions and removals follow documented entry, deprecation, and evidence-retention rules.

## Context
Platform APIs and permission behavior change independently of GHOSTRACE releases.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
