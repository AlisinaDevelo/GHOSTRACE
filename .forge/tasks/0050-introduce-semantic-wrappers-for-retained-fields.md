---
id: 0050
title: Introduce semantic wrappers for retained fields
status: done
agent: implementation-engineer
model: human
release: M1
depends_on: [0004, 0006]
change: null
workstream: foundation
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Implement the semantic identifier contract as typed constructors for source, application, repository, session, path-digest, bookmark, and reason fields.

## Acceptance criteria
- [x] Each wrapper validates canonical form, byte bounds, and forbidden-content sentinels.
- [x] Serde deserialization and programmatic construction share one validation path.
- [x] Mutation and property tests prove invalid values cannot be serialized after construction.

## Context
Typed wrappers turn documentation-only minimization rules into code-level invariants.

## Notes
Implemented in `96213c141c5e5603d4f4367566567c8fccd340d7` and verified in
`docs/evidence/0050-semantic-wrappers.md`. The wrappers preserve the string wire
format while making canonical, bounded, privacy-safe construction mandatory.
