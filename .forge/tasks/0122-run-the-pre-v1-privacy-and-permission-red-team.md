---
id: 0122
title: Run the pre-v1 privacy and permission red-team
status: backlog
agent: security-auditor
model: human
release: M6
parent: 0040
depends_on: [0033, 0037, 0115]
change: null
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Challenge every collector, local interface, export, diagnostic, installer, upgrade, and failure path against the published negative data and permission contract.

## Acceptance criteria
- [ ] The review includes prohibited-data sentinels, hostile local processes, malformed sources, permission drift, private contexts, symlink races, and downgrade attempts.
- [ ] Each finding has affected versions, severity, exploit preconditions, evidence, remediation, regression coverage, and disclosure handling.
- [ ] The refreshed threat model lists residual risks and does not close the gate on unaccepted high-severity findings.

## Context
The v1 boundary is the first point where all privacy-sensitive surfaces coexist.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
