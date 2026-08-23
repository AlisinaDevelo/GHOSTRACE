---
id: 0086
title: Implement retention planning and dry-run
status: backlog
agent: database-expert
model: human
release: M3
parent: 0021
depends_on: [0009, 0018, 0020]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Translate time, source, root, size, event-count, and legal-hold non-goals into a deterministic local retention plan before deletion.

## Acceptance criteria
- [ ] Dry-run reports affected counts, ranges, sources, key generations, gaps, and estimated reclaimed space.
- [ ] Policies define precedence and never silently treat an export as a backup or legal hold.
- [ ] Concurrent ingest and policy updates cannot expand the deletion set after confirmation.

## Context
Retention is a privacy feature but destructive scope must be stable and inspectable.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
