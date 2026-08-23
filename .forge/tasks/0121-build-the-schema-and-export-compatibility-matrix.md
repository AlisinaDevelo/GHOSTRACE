---
id: 0121
title: Build the schema and export compatibility matrix
status: backlog
agent: test-engineer
model: human
release: M6
parent: 0040
depends_on: [0020, 0021, 0083, 0084]
change: null
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Prove supported readers, writers, migrations, exports, and imports across every retained public schema and release line.

## Acceptance criteria
- [ ] Golden artifacts from every supported version run through upgrade, query, explanation, export, verification, and deletion paths.
- [ ] Forward, backward, unknown, mixed, corrupted, and partially migrated cases have explicit accept or refuse outcomes.
- [ ] Removing compatibility requires a deprecation window, migration tool, rollback evidence, and release-note impact statement.

## Context
Version fields are useful only when compatibility behavior is continuously exercised.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
