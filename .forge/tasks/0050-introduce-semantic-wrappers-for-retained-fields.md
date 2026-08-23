---
id: 0050
title: Introduce semantic wrappers for retained fields
status: ready
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
- [ ] Each wrapper validates canonical form, byte bounds, and forbidden-content sentinels.
- [ ] Serde deserialization and programmatic construction share one validation path.
- [ ] Mutation and property tests prove invalid values cannot be serialized after construction.

## Context
Typed wrappers turn documentation-only minimization rules into code-level invariants.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
