---
id: 0089
title: Add signed checkpoints and bounded repair workflows
status: backlog
agent: incident-responder
model: human
release: M3
parent: 0023
depends_on: [0088]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Create local verification checkpoints and an explicit repair mode that preserves corruption evidence instead of silently rewriting history.

## Acceptance criteria
- [ ] Checkpoints bind chain position, database identity, schema, key generation, policy set, and verification time.
- [ ] Repair operates on a verified copy, emits before-and-after manifests, and records every dropped or reconstructed interval as a gap.
- [ ] The normal writer refuses a journal with unresolved integrity failures.

## Context
Tamper detection needs an operational response that separates recovery from normal ingestion.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
