---
id: 0080
title: Define a bounded evidence-claim grammar
status: done
agent: implementation-engineer
model: human
release: M3
parent: 0019
depends_on: [0017, 0018, 0046]
change: pr-283
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Constrain explanations to versioned claim templates whose wording matches direct, contextual, inferred, conflicting, and unknown evidence.

## Acceptance criteria
- [x] Each template declares required facts, prohibited implications, evidence level, and gap behavior.
- [x] No template asserts intent, completeness, process attribution, or old-to-new rename identity without the required direct source.
- [x] Localization and rendering tests preserve claim meaning and cited event identifiers.

## Context
Free-form narrative makes it too easy to convert correlation or absence into an unsupported causal statement.

## Notes
Implemented by PR #283 and evidenced by
[`docs/evidence/0080-bounded-evidence-claim-grammar.md`](../../docs/evidence/0080-bounded-evidence-claim-grammar.md).
Completion requires the acceptance evidence above; issue closure alone is not evidence.
