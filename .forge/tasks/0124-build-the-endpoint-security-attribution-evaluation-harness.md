---
id: 0124
title: Build the Endpoint Security attribution evaluation harness
status: backlog
agent: security-researcher
model: human
release: M6
parent: 0042
depends_on: [0017, 0019, 0037]
change: null
workstream: release-scale
type: spike
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Measure whether an optional notify-only Endpoint Security mode adds reliable actor evidence worth its entitlement, permission, volume, and privacy cost.

## Acceptance criteria
- [ ] The harness records entitlement and authorization availability, event classes, mute rules, PID and process identity, selected-root filtering, drops, overhead, and false attribution.
- [ ] Ground-truth workloads compare FSEvents-only, Endpoint Security-only, and combined explanations with explicit precision and coverage limits.
- [ ] The result is a signed go, constrained-go, or no-go ADR; unavailable entitlement paths retain an evidence record rather than being skipped.

## Context
Endpoint Security must remain optional and cannot silently become necessary for the lower-permission baseline.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
