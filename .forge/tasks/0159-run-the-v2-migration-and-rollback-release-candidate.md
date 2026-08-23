---
id: 0159
title: Run the v2 migration and rollback release candidate
status: backlog
agent: release-engineer
model: human
release: M11
parent: 0153
depends_on: [0154, 0155, 0156, 0158]
change: null
workstream: long-term
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Exercise representative supported v1 histories through backup, upgrade, migration, verification, use, rollback window, export, repair, and uninstall on release-candidate artifacts.

## Acceptance criteria
- [ ] The matrix includes small, large, sparse, gap-heavy, multiple-key, multiple-policy, old-schema, adapter, and intentionally damaged synthetic journals.
- [ ] Every case records time, space, downtime, prompts, permissions, data changes, verification, failure, and recovery outcomes.
- [ ] The candidate cannot release when a supported input lacks a safe documented upgrade, stay, export, or recovery path.

## Context
Migration readiness is proven on accumulated histories, not a fresh database.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
