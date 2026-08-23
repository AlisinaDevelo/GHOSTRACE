---
id: 0153
title: Pass the v2 and long-term support readiness gate
status: backlog
agent: tech-lead
model: human
release: M11
depends_on: [0146, 0154, 0155, 0156, 0157, 0158, 0159, 0160]
change: null
workstream: long-term
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Close the 2026–2031 program with a migration-safe v2 decision, verified long-term data and key handling, independent assurance, and an explicit sustainability plan.

## Acceptance criteria
- [ ] Every M11 child issue has current acceptance evidence and all supported v1 installations have a tested upgrade, stay, export, or retirement path.
- [ ] Independent privacy and security findings are resolved or formally accepted with user-visible impact and no unaccepted critical or high risk.
- [ ] The release register states exactly what remains supported through 2032 and what is research rather than commitment.

## Context
The final gate protects years of local evidence from format, cryptographic, operational, and project-governance abandonment.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
