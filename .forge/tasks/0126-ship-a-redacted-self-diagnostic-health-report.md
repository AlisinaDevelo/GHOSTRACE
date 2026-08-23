---
id: 0126
title: Ship a redacted self-diagnostic health report
status: backlog
agent: sre
model: human
release: M7
parent: 0125
depends_on: [0040]
change: null
workstream: operations
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Give users an offline way to inspect collector, policy, key, journal, cursor, gap, service, permission, version, and update health without exposing retained evidence.

## Acceptance criteria
- [ ] The report schema contains only enumerated status, counts, bounded timings, versions, and one-way identities.
- [ ] A prohibited-data corpus proves no paths, events, origins, commands, titles, credentials, or raw errors appear.
- [ ] Human and machine-readable forms provide actionable local remediation and explicit unknown states.

## Context
Support evidence should explain system health without becoming a secondary sensitive journal.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
