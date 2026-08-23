---
id: 0140
title: Publish a synthetic causal ground-truth corpus
status: backlog
agent: researcher
model: human
release: M9
parent: 0139
depends_on: [0132]
change: null
workstream: research
type: test
priority: p1
risks: [privacy]
platform: macos
---

## Goal
Create diverse, deterministic, non-user macOS workflows with known operations, actors, timing, source availability, interruptions, and causal relationships.

## Acceptance criteria
- [ ] The corpus covers file editors, shells, Git, builds, browsers, services, concurrent applications, failures, policy denials, and missing sources.
- [ ] Ground truth separates operation causality from what each collector is permitted and able to observe.
- [ ] Generators, seeds, environment manifests, expected evidence, and licenses permit independent reproduction.

## Context
Evaluation needs ground truth that is richer than the journal and does not leak real user activity.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
