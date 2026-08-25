---
id: 0081
title: Version the cross-source correlation rule registry
status: done
agent: implementation-engineer
model: human
release: M3
parent: 0019
depends_on: [0079, 0080]
change: pr-285
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Represent every correlation rule as inspectable inputs, bounds, exclusions, evidence output, version, and counterexample set.

## Acceptance criteria
- [x] Rules cannot read fields outside the query policy or convert an unknown source interval into positive evidence.
- [x] Each rule includes positive, negative, ambiguous, adversarial, and clock-skew fixtures.
- [x] Changing a rule version changes explanation identity and remains reproducible for historical exports.

## Context
Correlation logic is part of the evidence model and must be versioned like a schema.

## Notes
Implemented by PR #285 and evidenced by
[`docs/evidence/0081-versioned-correlation-rule-registry.md`](../../docs/evidence/0081-versioned-correlation-rule-registry.md).
Completion requires the acceptance evidence above; issue closure alone is not evidence.
