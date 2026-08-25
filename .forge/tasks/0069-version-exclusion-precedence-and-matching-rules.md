---
id: 0069
title: Version exclusion precedence and matching rules
status: done
agent: privacy-engineer
model: human
release: M2
parent: 0014
depends_on: [0007, 0050]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Specify how root, subtree, file-kind, application, temporary-file, VCS, and user-defined exclusions combine before persistence.

## Acceptance criteria
- [x] A deterministic precedence table covers allow, deny, redact, and summarize outcomes.
- [x] Policy updates re-evaluate future events only and preserve the version used for existing evidence.
- [x] Property tests cover overlapping, nested, escaped, case-variant, and empty patterns without catastrophic matching time.

## Context
Exclusions are a privacy control and must not depend on rule ordering accidents.

## Notes
Implemented as the bounded `exclusion-policy-v1` engine. Safety action precedence is
`deny > redact > summarize > allow`; rule-class precedence is user > subtree > root
> application > file kind > temporary file > VCS, followed by literal specificity.
Patterns use a linear-time glob matcher with explicit escapes and bounded inputs.
`ExclusionPolicyHistory` keeps validated versions for recorded evidence and applies
new versions only to future subjects. See `docs/evidence/0069-exclusion-matching.md`
for the target-device receipts and limitations.
