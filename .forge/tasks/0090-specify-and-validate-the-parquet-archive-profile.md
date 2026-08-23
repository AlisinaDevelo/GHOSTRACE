---
id: 0090
title: Specify and validate the Parquet archive profile
status: backlog
agent: data-engineer
model: human
release: M3
parent: 0022
depends_on: [0084, 0087]
change: null
workstream: explain-export
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define an optional analytical archive that preserves evidence semantics, minimizes plaintext exposure, and remains subordinate to the canonical journal and manifest.

## Acceptance criteria
- [ ] Column types, nullability, schema evolution, ordering, gap, provenance, and policy mappings are versioned.
- [ ] Streaming export and validation remain bounded and reject undeclared or lossy conversions.
- [ ] Threat documentation covers column statistics, metadata leakage, temporary files, compression, and downstream deletion limits.

## Context
Parquet is useful for offline analysis but its metadata and tooling create a wider plaintext boundary.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
