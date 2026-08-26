---
id: 0019
title: Implement deterministic evidence-backed explain
status: done
agent: maintainer
model: human
release: M3
depends_on: [0017, 0018, 0080, 0081, 0082]
change: pr-298
workstream: explain-export
type: feature
priority: p1
risks: [security]
platform: any
---

## Goal
Explain observed change sequences with reproducible statements that remain traceable to journal evidence and visible coverage limits.

## Acceptance criteria
- [x] Every statement cites event IDs.
- [x] Direct and inferred facts are labeled.
- [x] Gaps and coverage are shown.
- [x] Identical input produces identical output.
- [x] Explanation has no LLM dependency.

## Context
The product may describe supported sequences and correlations, but it must not manufacture causality or hide missing source coverage.

## Notes
The parent capability is implemented by the bounded claim grammar, correlation
registry, and explanation determinism child work in PRs #283, #285, and #287.
The retained parent acceptance record is
`docs/evidence/0019-evidence-backed-explain.md`; issue #23 closes only after
that evidence change is merged and independently verified.
