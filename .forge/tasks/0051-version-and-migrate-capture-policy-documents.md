---
id: 0051
title: Version and migrate capture-policy documents
status: ready
agent: architect
model: human
release: M1
parent: 0007
depends_on: [0004, 0006]
change: null
workstream: privacy
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Define a strict policy-document schema with immutable identity, monotonic versions, migration rules, and fail-closed handling of unknown fields.

## Acceptance criteria
- [ ] Policy schema versions have golden valid and invalid documents.
- [ ] Upgrades preserve user choices or require explicit reconfirmation when semantics change.
- [ ] Unknown versions, duplicated identities, and downgrade attempts refuse capture without retaining the candidate observation.

## Context
Policy history is evidence and must remain interpretable across upgrades.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
