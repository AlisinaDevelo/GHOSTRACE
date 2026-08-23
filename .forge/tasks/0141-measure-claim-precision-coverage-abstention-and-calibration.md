---
id: 0141
title: Measure claim precision, coverage, abstention, and calibration
status: backlog
agent: researcher
model: human
release: M9
parent: 0139
depends_on: [0140]
change: null
workstream: research
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Evaluate each explanation rule and source combination against ground truth while rewarding supported abstention and penalizing unsupported causal claims.

## Acceptance criteria
- [ ] Metrics are reported by claim type, evidence level, workload, source coverage, gap condition, OS version, and policy profile.
- [ ] False-positive, false-negative, conflict, abstention, and unsupported-completeness cases retain representative counterexamples.
- [ ] Thresholds are release targets justified by user risk and do not collapse evidence classes into one score.

## Context
High answer rate is harmful if the product fills source gaps with plausible stories.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
