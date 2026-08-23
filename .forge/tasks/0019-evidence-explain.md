---
id: 0019
title: Implement deterministic evidence-backed explain
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0017, 0018]
change: null
---

## Goal
Explain observed change sequences with reproducible statements that remain traceable to journal evidence and visible coverage limits.

## Acceptance criteria
- [ ] Every statement cites event IDs.
- [ ] Direct and inferred facts are labeled.
- [ ] Gaps and coverage are shown.
- [ ] Identical input produces identical output.
- [ ] Explanation has no LLM dependency.

## Context
The product may describe supported sequences and correlations, but it must not manufacture causality or hide missing source coverage.

## Notes
The fixture-only headstart already produces deterministic parent-chain explanations with event citations, evidence labels, and visible gaps without an LLM. The live-query and filesystem-evaluation dependencies remain open.
