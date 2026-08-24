---
id: 0049
title: Make ingestion origin an explicit capability
status: done
agent: security-auditor
model: human
release: M1
parent: 0010
depends_on: [0004, 0006]
change: null
workstream: storage
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Prevent callers from forging live, fixture, imported, or repaired provenance by binding ingestion to an unforgeable adapter-origin capability.

## Acceptance criteria
- [x] The public journal API accepts typed origin capabilities rather than provenance strings.
- [x] Fixture, live, import, and repair origins have distinct construction paths and allowed event classes.
- [x] Tests prove a deserialized fixture cannot claim a live collector identity through the generic API.

## Context
Adapter provenance must be enforced at the journal boundary before the library becomes a public integration surface.

## Notes
Implemented in `e926547e1790cbf80afb4ac5aaefd71ed98e778e` and verified in
`docs/evidence/0049-ingestion-origin-capability.md`. The journal now requires a
sealed `IngestionOrigin`; fixture deserialization cannot cross into the live
provenance namespace.
