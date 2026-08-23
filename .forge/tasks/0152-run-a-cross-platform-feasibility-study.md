---
id: 0152
title: Run a cross-platform feasibility study
status: backlog
agent: architect
model: human
release: M10
parent: 0146
depends_on: [0139, 0147]
change: null
workstream: ecosystem
type: spike
priority: p2
risks: [privacy, security]
platform: any
---

## Goal
Evaluate whether Linux or Windows can support the same consent, source-limit, gap, local-storage, key, lifecycle, and explanation contracts without redefining the macOS product.

## Acceptance criteria
- [ ] The study maps candidate event sources, permissions, cursor semantics, loss modes, secure storage, packaging, and support burden.
- [ ] Small prototypes measure evidence quality and privilege footprint against the macOS baseline.
- [ ] The ADR records go, later, or no-go per platform and creates no compatibility promise without funded implementation and test plans.

## Context
Cross-platform expansion is a new product boundary, not a conditional compilation exercise.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
