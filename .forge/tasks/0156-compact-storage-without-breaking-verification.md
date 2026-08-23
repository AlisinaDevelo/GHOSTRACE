---
id: 0156
title: Compact storage without breaking verification
status: backlog
agent: database-expert
model: human
release: M11
parent: 0153
depends_on: [0087, 0089, 0144]
change: null
workstream: long-term
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Reclaim space and optimize long-lived journals while preserving event identity, gap semantics, retention evidence, authenticated ordering, and compatibility.

## Acceptance criteria
- [ ] The compaction plan declares which bytes, indexes, tombstones, checkpoints, and verification proofs change or remain stable.
- [ ] Dry-run, copied-database execution, power loss, disk full, concurrent read, rollback, and post-compaction verification are tested.
- [ ] Compaction cannot imply secure erasure beyond the documented database, filesystem, backup, and key boundaries.

## Context
Physical database maintenance must not silently rewrite the logical evidence contract.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
