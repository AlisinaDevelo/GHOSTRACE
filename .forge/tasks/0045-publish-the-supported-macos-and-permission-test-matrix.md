---
id: 0045
title: Publish the supported macOS and permission test matrix
status: ready
agent: platform-engineer
model: human
release: M0
depends_on: [0002]
change: null
workstream: foundation
type: docs
priority: p0
risks: [privacy]
platform: macos
---

## Goal
Define the operating-system, architecture, filesystem, login-session, and permission combinations that each release must test or explicitly refuse.

## Acceptance criteria
- [ ] The matrix names supported macOS major versions and Intel and Apple silicon expectations.
- [ ] Each collector lists required, optional, and prohibited permissions with observable refusal behavior.
- [ ] Annual macOS beta and release-candidate validation has an owner, evidence format, and retirement rule.

## Context
Platform support is an evidence contract, not an implication from whichever runner happened to pass.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
