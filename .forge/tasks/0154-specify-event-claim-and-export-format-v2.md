---
id: 0154
title: Specify event, claim, and export format v2
status: backlog
agent: architect
model: human
release: M11
parent: 0153
depends_on: [0145, 0147]
change: null
workstream: long-term
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Decide whether accumulated evidence requires a v2 format and define only changes justified by measured compatibility, privacy, performance, or interoperability limits.

## Acceptance criteria
- [ ] A requirements report traces every proposed change to a reproduced limitation and rejected backward-compatible alternative.
- [ ] Canonical encoding, schema identifiers, evidence levels, gaps, policy, provenance, ordering, unknown fields, and extension points are specified together.
- [ ] v1-to-v2, v2-to-v1-loss report, mixed-version refusal, and long-lived golden artifacts pass independent implementations where practical.

## Context
A major version is a migration cost and must not be used as a general cleanup milestone.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
