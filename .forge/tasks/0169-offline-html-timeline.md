---
id: 0169
title: Render an offline HTML timeline report
status: done
agent: frontend-specialist
model: human
release: M4
depends_on: [0165, 0019]
change: null
workstream: explain-export
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Produce a single self-contained HTML file that shows a journal's timeline, evidence levels, gaps, and explanations for a person to read.

## Acceptance criteria
- [x] The report is generated only on request, works offline with no external scripts or fonts, and states that it is a plaintext disclosure.
- [x] Gaps, abstentions, and evidence levels are visually distinct and never rendered as complete coverage.
- [x] The file contains no raw paths, URLs beyond canonical origins, command text, or secrets, and a sentinel test proves it.

## Context
Explanations are JSON today; people need a readable view that keeps the evidence honesty visible.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
