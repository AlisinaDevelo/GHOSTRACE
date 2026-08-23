---
id: 0080
title: Define a bounded evidence-claim grammar
status: backlog
agent: researcher
model: human
release: M3
parent: 0019
depends_on: [0017, 0018, 0046]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Constrain explanations to versioned claim templates whose wording matches direct, contextual, inferred, conflicting, and unknown evidence.

## Acceptance criteria
- [ ] Each template declares required facts, prohibited implications, evidence level, and gap behavior.
- [ ] No template asserts intent, completeness, process attribution, or old-to-new rename identity without the required direct source.
- [ ] Localization and rendering tests preserve claim meaning and cited event identifiers.

## Context
Free-form narrative makes it too easy to convert correlation or absence into an unsupported causal statement.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
