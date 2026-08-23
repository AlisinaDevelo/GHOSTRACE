---
id: 0082
title: Build explanation determinism and counterexample tests
status: backlog
agent: test-engineer
model: human
release: M3
parent: 0019
depends_on: [0080, 0081]
change: null
workstream: explain-export
type: test
priority: p0
risks: [security]
platform: any
---

## Goal
Prove that identical committed evidence and rule versions produce byte-stable claims while near-miss evidence does not overstate causality.

## Acceptance criteria
- [ ] Golden cases cover every claim template, evidence level, gap interaction, and conflict outcome.
- [ ] Property tests permute ingestion, equal timestamps, irrelevant events, and page boundaries without changing supported claims.
- [ ] Mutation tests demonstrate that removing a required observation downgrades or removes the claim.

## Context
Determinism and counterexamples are necessary to audit an explanation engine over time.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
