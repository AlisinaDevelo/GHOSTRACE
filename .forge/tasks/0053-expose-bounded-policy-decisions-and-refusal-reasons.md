---
id: 0053
title: Expose bounded policy decisions and refusal reasons
status: done
agent: security-auditor
model: human
release: M1
parent: 0007
depends_on: [0004, 0006]
change: null
workstream: privacy
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Make allow, deny, redact, summarize, and refuse outcomes explainable without logging the rejected sensitive observation.

## Acceptance criteria
- [x] Decision records use a finite reason-code registry and bounded public metadata.
- [x] Diagnostics distinguish policy denial, malformed input, unsupported scope, and internal failure.
- [x] Adversarial tests prove errors and debug output do not echo paths, secrets, or rejected payloads.

## Context
A visible refusal must still preserve the data-minimization boundary.

## Notes
Implemented in `b4ae45fbd5958a79e60da85bff33f06cf128a01b` and verified in
`docs/evidence/0053-bounded-policy-decisions.md`. Completion evidence covers finite
outcomes, diagnostic classes, and adversarial non-echo behavior.
