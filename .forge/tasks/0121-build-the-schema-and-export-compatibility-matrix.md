---
id: 0121
title: Build the schema and export compatibility matrix
status: done
agent: test-engineer
model: human
release: M6
parent: 0040
depends_on: [0020, 0021, 0083, 0084]
change: pr-360
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Prove supported readers, writers, migrations, exports, and imports across every retained public schema and release line.

## Acceptance criteria
- [x] Golden artifacts from every supported version run through upgrade, query, explanation, export, verification, and deletion paths.
- [x] Forward, backward, unknown, mixed, corrupted, and partially migrated cases have explicit accept or refuse outcomes.
- [x] Removing compatibility requires a deprecation window, migration tool, rollback evidence, and release-note impact statement.

## Context
Version fields are useful only when compatibility behavior is continuously exercised.

## Notes
Implemented in PR #360 and squash-merged to protected `main` at
`47645a616ab7fb178d424a24b09e3aeaf00bbc94`. Verified on merged `main`; see
[`docs/evidence/0121-compatibility-matrix.md`](../../docs/evidence/0121-compatibility-matrix.md)
for command-level device receipts and limitations.
