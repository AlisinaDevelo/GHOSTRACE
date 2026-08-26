---
id: 0090
title: Specify and validate the Parquet archive profile
status: done
agent: data-engineer
model: human
release: M3
parent: 0022
depends_on: [0084, 0087]
change: pr-314
workstream: explain-export
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define an optional analytical archive that preserves evidence semantics, minimizes plaintext exposure, and remains subordinate to the canonical journal and manifest.

## Acceptance criteria
- [x] Column types, nullability, schema evolution, ordering, gap, provenance, and policy mappings are versioned.
- [x] Streaming export and validation remain bounded and reject undeclared or lossy conversions.
- [x] Threat documentation covers column statistics, metadata leakage, temporary files, compression, and downstream deletion limits.

## Context
Parquet is useful for offline analysis but its metadata and tooling create a wider plaintext boundary.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.

Implemented in PR #314 and merged to protected `main` at
`e2e811c7ca40d2c4b001166659c7f76a321cb5de`. Completion evidence is retained in
`docs/evidence/0090-parquet-archive-profile.md`.
