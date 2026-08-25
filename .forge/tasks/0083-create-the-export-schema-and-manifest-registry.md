---
id: 0083
title: Create the export schema and manifest registry
status: done
agent: implementation-engineer
model: human
release: M3
parent: 0020
depends_on: [0018, 0019, 0046]
change: pr-289
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Publish versioned machine-readable contracts for event, gap, claim, policy, source-coverage, and export-manifest records.

## Acceptance criteria
- [x] Every schema has a stable identifier, compatibility class, strict unknown-field behavior, and golden examples.
- [x] A manifest binds record counts, byte counts, digests, schema versions, query scope, policy profiles, gaps, and tool version.
- [x] Validators reject mixed or undeclared versions before any consumer treats an export as complete.

## Context
Portability requires a registry for all evidence records, not only the event envelope.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
