---
id: 0146
title: Pass the governed ecosystem extensibility gate
status: backlog
agent: tech-lead
model: human
release: M10
depends_on: [0139, 0147, 0148, 0149, 0150, 0151, 0152]
change: null
workstream: ecosystem
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Enable a small governed adapter ecosystem only after stable contracts, isolation, signing, revocation, conformance, response, and platform-scope evidence are ready.

## Acceptance criteria
- [ ] All M10 child issues close with reviewed evidence and no unresolved high-severity ecosystem finding.
- [ ] Third-party code cannot bypass policy, claim native provenance, access arbitrary journal data, or add undeclared networking and permissions.
- [ ] The governance model can revoke unsafe adapters and communicate impact without remotely controlling user data.

## Context
Extensibility multiplies trust boundaries and therefore follows, rather than precedes, mature conformance and evaluation.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
