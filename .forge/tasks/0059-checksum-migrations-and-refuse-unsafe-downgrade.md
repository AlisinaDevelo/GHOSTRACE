---
id: 0059
title: Checksum migrations and refuse unsafe downgrade
status: done
agent: database-expert
model: human
release: M1
parent: 0009
depends_on: [0006, 0057]
change: null
workstream: storage
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Make database evolution deterministic, tamper-evident, crash-safe, and explicit about backward incompatibility.

## Acceptance criteria
- [x] Applied migrations record stable identifiers, checksums, schema versions, and tool versions.
- [x] Modified, missing, reordered, partially applied, or future migrations refuse normal startup.
- [x] Upgrade, crash-at-each-step, backup restore, and unsupported downgrade fixtures run in CI.

## Context
A privacy-sensitive local journal must not guess how to open an unknown or partially migrated schema.

## Notes
Implemented in PR #201 and merged to protected `main` at
`88dd03564deb995c037666bb17d90dbd877a2151`. The journal records ordered
migration identifiers, SHA-256 SQL checksums, schema versions, tool versions, and
transactional application timestamps. It upgrades the legacy v1 fixture schema and
refuses modified, missing, reordered, future, partial, and unsupported-downgrade
state. Source and merged-main device evidence is retained in
`docs/evidence/0059-migration-ledger.md`.
