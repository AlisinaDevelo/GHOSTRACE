---
id: 0126
title: Ship a redacted self-diagnostic health report
status: in-progress
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
Implementing bounded key-free offline inspection with fixed status/remediation
codes and aggregate counts. Unknown runtime, permissions, keys and update state
must remain explicitly not checked. Dependency 0040 remains a release gate;
implementation and local acceptance do not close the operational program.
