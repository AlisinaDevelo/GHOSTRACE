---
id: 0085
title: Add export redaction preview and policy receipts
status: backlog
agent: privacy-engineer
model: human
release: M3
parent: 0020
depends_on: [0007, 0083]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Show exactly which sources, fields, time ranges, profiles, gaps, and record counts will leave the encrypted journal before writing plaintext.

## Acceptance criteria
- [ ] Preview and execution use one immutable query and redaction plan digest.
- [ ] The preview warns that plaintext metadata will leave encrypted storage and requires an explicit confirmation bound to that plan digest.
- [ ] A policy change or journal snapshot change between preview and execution requires reconfirmation or produces a clearly bounded delta.
- [ ] The receipt records destination class and manifest digest without retaining the destination path in normal diagnostics.

## Context
Explicit export is a declassification action and deserves a reviewable plan.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
