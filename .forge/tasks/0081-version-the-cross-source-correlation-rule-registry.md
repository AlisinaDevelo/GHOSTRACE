---
id: 0081
title: Version the cross-source correlation rule registry
status: backlog
agent: architect
model: human
release: M3
parent: 0019
depends_on: [0079, 0080]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Represent every correlation rule as inspectable inputs, bounds, exclusions, evidence output, version, and counterexample set.

## Acceptance criteria
- [ ] Rules cannot read fields outside the query policy or convert an unknown source interval into positive evidence.
- [ ] Each rule includes positive, negative, ambiguous, adversarial, and clock-skew fixtures.
- [ ] Changing a rule version changes explanation identity and remains reproducible for historical exports.

## Context
Correlation logic is part of the evidence model and must be versioned like a schema.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
