---
id: 0053
title: Expose bounded policy decisions and refusal reasons
status: ready
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
- [ ] Decision records use a finite reason-code registry and bounded public metadata.
- [ ] Diagnostics distinguish policy denial, malformed input, unsupported scope, and internal failure.
- [ ] Adversarial tests prove errors and debug output do not echo paths, secrets, or rejected payloads.

## Context
A visible refusal must still preserve the data-minimization boundary.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
