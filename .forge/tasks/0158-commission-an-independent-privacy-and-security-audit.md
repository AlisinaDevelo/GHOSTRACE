---
id: 0158
title: Commission an independent privacy and security audit
status: backlog
agent: security-auditor
model: human
release: M11
parent: 0153
depends_on: [0146, 0154, 0155, 0156]
change: null
workstream: long-term
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Obtain independent review of architecture, Rust unsafe code, cryptography, Keychain, storage, collectors, browser host, service, UI, updates, adapters, formats, and incident controls.

## Acceptance criteria
- [ ] The scope, commit, binaries, configurations, threat assumptions, test access, exclusions, and auditor independence are public.
- [ ] Findings receive severity, affected versions, remediation, regression evidence, disclosure status, and residual-risk decisions.
- [ ] A focused re-test verifies fixes and the project does not market an audit beyond its actual scope and date.

## Context
Internal green checks are not independent assurance for a mature privacy-sensitive system.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
