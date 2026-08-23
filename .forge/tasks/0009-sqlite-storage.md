---
id: 0009
title: Create SQLite WAL schema and migration runner
status: backlog
agent: maintainer
model: human
release: M1
depends_on: [0006, 0008]
change: null
---

## Goal
Create the durable local schema and repeatable migration path for events, collector progress, policy state, and diagnostics.

## Acceptance criteria
- [ ] Events, cursors, policy metadata, diagnostics, and schema-version tables exist.
- [ ] WAL mode, synchronous FULL, and foreign-key settings are enforced.
- [ ] Journal directories and files use 0700 and 0600 permissions respectively.
- [ ] Migrations are idempotent.

## Context
Use one writer and read-only readers. Schema migrations must be checked into the repository and remain testable from an empty database.

## Notes
The headstart includes the initial SQLite tables, checked-in idempotent migration, WAL/FULL/foreign-key settings, and Unix file-permission checks. Production Keychain dependency and complete directory/sidecar hardening keep this task open.
