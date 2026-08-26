---
id: 0023
title: Add tamper-evident event chain and verifier
status: done
agent: maintainer
model: human
release: M3
depends_on: [0008, 0009, 0020, 0088, 0089]
change: pr-312
workstream: explain-export
type: feature
priority: p1
risks: [security]
platform: any
---

## Goal
Make offline changes to stored event history detectable while preserving clear limits on what the journal can prove.

## Acceptance criteria
- [x] The verifier detects payload edits, deletion, reorder, and replay.
- [x] Anchor handling survives key rotation.
- [x] Documentation makes no legal chain-of-custody claim.

## Context
Tamper evidence supports personal confidence and investigation integrity; it does not establish complete collection or protection from a compromised account.

## Notes
Implementation PR [#312](https://github.com/AlisinaDevelo/GHOSTRACE/pull/312)
merged to protected `main` at
`98256981b0ba3faf47c09bf74570e577c77c3738`. Device reproduction and
acceptance mapping are retained in
`docs/evidence/0023-tamper-evident-chain.md`.
