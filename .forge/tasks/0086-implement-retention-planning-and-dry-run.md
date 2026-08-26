---
id: 0086
title: Implement retention planning and dry-run
status: done
agent: database-expert
model: human
release: M3
parent: 0021
depends_on: [0009, 0018, 0020]
change: pr-302
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Translate time, source, root, size, event-count, and legal-hold non-goals into a deterministic local retention plan before deletion.

## Acceptance criteria
- [x] Dry-run reports affected counts, ranges, sources, key generations, gaps, and estimated reclaimed space.
- [x] Policies define precedence and never silently treat an export as a backup or legal hold.
- [x] Concurrent ingest and policy updates cannot expand the deletion set after confirmation.

## Context
Retention is a privacy feature but destructive scope must be stable and inspectable.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Implemented and reviewed in PR
[#302](https://github.com/AlisinaDevelo/GHOSTRACE/pull/302), with the retained
acceptance record at `docs/evidence/0086-retention-planning.md`. The destructive
deletion command remains a separate parent-task gate and must enforce the stored
confirmation before mutating rows.
