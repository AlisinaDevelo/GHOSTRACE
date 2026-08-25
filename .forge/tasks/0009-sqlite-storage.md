---
id: 0009
title: Create SQLite WAL schema and migration runner
status: done
agent: maintainer
model: human
release: M1
depends_on: [0006, 0008, 0057, 0058, 0059]
change: pr-229-16ce399eab65f3fc5919cab07003e7eb37d35900
workstream: storage
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Create the durable local schema and repeatable migration path for events, collector progress, policy state, and diagnostics.

## Acceptance criteria
- [x] Events, cursors, policy metadata, diagnostics, and schema-version tables exist.
- [x] WAL mode, synchronous FULL, and foreign-key settings are enforced.
- [x] Journal directories and files use 0700 and 0600 permissions respectively.
- [x] Migrations are idempotent.

## Context
Use one writer and read-only readers. Schema migrations must be checked into the repository and remain testable from an empty database.

## Notes
The headstart includes the initial SQLite tables, checked-in idempotent migration,
WAL/FULL/foreign-key settings, and Unix file-permission checks. This review adds
a focused file-backed contract test for schema pragmas, 0700/0600 permissions,
cursor/policy/diagnostic durability, and migration-ledger reuse. Protected-main
rerun evidence is complete on the protected merge and retained below.
