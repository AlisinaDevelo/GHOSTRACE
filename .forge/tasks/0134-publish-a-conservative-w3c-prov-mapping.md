---
id: 0134
title: Publish a conservative W3C PROV mapping
status: backlog
agent: researcher
model: human
release: M8
parent: 0132
depends_on: [0125, 0133]
change: null
workstream: interoperability
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Map exportable GHOSTRACE entities, activities, agents, derivations, associations, timestamps, evidence levels, and gaps to a documented PROV profile without strengthening claims.

## Acceptance criteria
- [ ] The profile declares exact, lossy, unsupported, and extension mappings with machine-readable context and examples.
- [ ] Direct, contextual, inferred, conflicting, unknown, and gap semantics survive round-trip or cause explicit loss warnings.
- [ ] A validator rejects mappings that invent agents, causation, attribution, or coverage absent from the source journal.

## Context
W3C PROV supports interchange, but its vocabulary must not turn local observations into stronger provenance than recorded.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
