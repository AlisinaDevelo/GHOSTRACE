---
id: 0149
title: Sign, admit, quarantine, and revoke adapters
status: backlog
agent: security-auditor
model: human
release: M10
parent: 0146
depends_on: [0147, 0148]
change: null
workstream: ecosystem
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Bind adapter code, manifest, developer identity, conformance evidence, policy capabilities, compatibility, and revocation state before activation.

## Acceptance criteria
- [ ] Admission verifies code identity and signatures using a documented trust policy that also supports local development explicitly.
- [ ] Changed, expired, revoked, quarantined, incompatible, or uncertified adapters stay disabled without deleting user evidence.
- [ ] Revocation data can be distributed as an explicit signed update without silently uploading inventory or activity.

## Context
Code signing identifies code under a trust policy; it does not replace capability or conformance checks.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
