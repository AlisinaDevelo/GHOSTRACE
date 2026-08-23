---
id: 0023
title: Add tamper-evident event chain and verifier
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0008, 0009, 0020, 0088, 0089]
change: null
workstream: explain-export
type: feature
priority: p1
risks: [security]
platform: any
---

## Goal
Make offline changes to stored event history detectable while preserving clear limits on what the journal can prove.

## Acceptance criteria
- [ ] The verifier detects payload edits, deletion, reorder, and replay.
- [ ] Anchor handling survives key rotation.
- [ ] Documentation makes no legal chain-of-custody claim.

## Context
Tamper evidence supports personal confidence and investigation integrity; it does not establish complete collection or protection from a compromised account.

## Notes
No implementation notes yet.
