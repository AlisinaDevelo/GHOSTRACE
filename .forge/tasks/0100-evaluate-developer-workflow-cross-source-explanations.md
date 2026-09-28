---
id: 0100
title: Evaluate developer-workflow cross-source explanations
status: done
agent: researcher
model: human
release: M4
parent: 0028
depends_on: [0092, 0096, 0099]
change: pr-366
workstream: frontmost
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Measure which shell, Git, frontmost, and filesystem observations support useful explanations without upgrading temporal context into actor attribution.

## Acceptance criteria
- [x] Synthetic workflows include build, test, checkout, rebase, editor save, generated files, and concurrent unrelated activity.
- [x] Ground truth labels supported, unsupported, conflicting, and unknowable claims for each source combination.
- [x] The report publishes precision, coverage, abstention, gap visibility, and representative counterexamples.

## Context
The first multi-source evaluation should reward honest abstention as well as useful supported explanations.

## Notes
Implemented in PR #366 and squash-merged to protected `main` at
`31168bce32085f56eee904fdd3157db5fe3a0586`. Verified on merged `main`; see
[`docs/evidence/0100-workflow-evaluation.md`](../../docs/evidence/0100-workflow-evaluation.md)
for command-level device receipts and limitations.
