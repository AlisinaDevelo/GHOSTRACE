---
id: 0046
title: Freeze semantic identifier and digest contracts
status: ready
agent: security-auditor
model: human
release: M0
depends_on: [0004, 0006]
change: null
workstream: foundation
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Replace structurally valid free-form identifiers with documented semantic formats before third-party or live data is accepted.

## Acceptance criteria
- [ ] Every public identifier and digest field has a type, canonical encoding, length bound, and sensitivity classification.
- [ ] Constructors reject paths, credentials, control characters, and ambiguous encodings in identifier-shaped fields.
- [ ] Schema, Rust validation, fixtures, and compatibility tests enforce the same contract.

## Context
This closes the gap between byte bounds and actual minimization semantics identified in the security review.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
