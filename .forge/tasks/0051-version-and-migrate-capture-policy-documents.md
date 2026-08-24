---
id: 0051
title: Version and migrate capture-policy documents
status: done
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
- [x] Policy schema versions have golden valid and invalid documents.
- [x] Upgrades preserve user choices or require explicit reconfirmation when semantics change.
- [x] Unknown versions, duplicated identities, and downgrade attempts refuse capture without retaining the candidate observation.

## Context
Policy history is evidence and must remain interpretable across upgrades.

## Notes
Implemented in `f114c1b8d57e0c69f7894fa885667885e08d1c40` and verified in
`docs/evidence/0051-policy-documents.md`. Policy documents are strict, versioned,
and migration-safe; semantic changes require explicit reconfirmation.
