---
id: 0114
title: Make launchd install, upgrade, and permission recovery reversible
status: backlog
agent: macos-engineer
model: human
release: M5
parent: 0037
depends_on: [0013, 0027, 0030, 0035]
change: null
workstream: service-ui
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Manage the per-user service lifecycle without privilege escalation, unsafe file replacement, silent capture re-enablement, or orphaned collectors.

## Acceptance criteria
- [ ] Plan, install, bootstrap, kickstart, pause, upgrade, rollback, bootout, and uninstall are idempotent and receipt-backed.
- [ ] Ownership, modes, signatures, paths, labels, environment, KeepAlive behavior, login sessions, and concurrent versions are validated.
- [ ] Missing permissions or keys produce visible degraded states and bounded gaps rather than restart loops or plaintext buffering.

## Context
The durable service is a user agent, not a root daemon, and its lifecycle must preserve current consent.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
