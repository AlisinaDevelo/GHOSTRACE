---
id: 0100
title: Evaluate developer-workflow cross-source explanations
status: backlog
agent: researcher
model: human
release: M4
parent: 0028
depends_on: [0092, 0096, 0099]
change: null
workstream: frontmost
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Measure which shell, Git, frontmost, and filesystem observations support useful explanations without upgrading temporal context into actor attribution.

## Acceptance criteria
- [ ] Synthetic workflows include build, test, checkout, rebase, editor save, generated files, and concurrent unrelated activity.
- [ ] Ground truth labels supported, unsupported, conflicting, and unknowable claims for each source combination.
- [ ] The report publishes precision, coverage, abstention, gap visibility, and representative counterexamples.

## Context
The first multi-source evaluation should reward honest abstention as well as useful supported explanations.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
