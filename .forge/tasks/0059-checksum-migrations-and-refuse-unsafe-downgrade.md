---
id: 0059
title: Checksum migrations and refuse unsafe downgrade
status: review
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
- [ ] Applied migrations record stable identifiers, checksums, schema versions, and tool versions.
- [ ] Modified, missing, reordered, partially applied, or future migrations refuse normal startup.
- [ ] Upgrade, crash-at-each-step, backup restore, and unsupported downgrade fixtures run in CI.

## Context
A privacy-sensitive local journal must not guess how to open an unknown or partially migrated schema.

## Notes
Implementation is under review in the 0059 pull request. The journal now records
ordered migration identifiers, SHA-256 SQL checksums, schema versions, tool
versions, and transactional application timestamps. It upgrades the legacy v1
fixture schema and refuses modified, missing, reordered, future, partial, and
unsupported-downgrade state. Device crash, restore, and refusal evidence is being
retained in `docs/evidence/0059-migration-ledger.md`; the task remains `review`
until the merged-main rerun is appended.
