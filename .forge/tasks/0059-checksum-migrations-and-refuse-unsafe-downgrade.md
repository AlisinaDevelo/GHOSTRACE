---
id: 0059
title: Checksum migrations and refuse unsafe downgrade
status: backlog
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
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
