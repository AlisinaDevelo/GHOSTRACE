---
id: 0077
title: Define snapshot-consistent query pagination
status: backlog
agent: database-expert
model: human
release: M3
parent: 0018
depends_on: [0010, 0012]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Return stable bounded pages from a growing journal without skipping, duplicating, or exposing events outside the authorized query scope.

## Acceptance criteria
- [ ] Page tokens bind query parameters, policy scope, schema version, snapshot boundary, and stable ordering keys.
- [ ] Expired, forged, cross-profile, or future tokens fail with bounded errors.
- [ ] Concurrent ingest, deletion, retention, and migration tests prove the documented snapshot semantics.

## Context
Offset pagination over a changing evidence journal is neither stable nor tamper-resistant.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
