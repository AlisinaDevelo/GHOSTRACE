---
id: 0012
title: Ship fixture ingest, explain, and JSONL export CLI slice
status: backlog
agent: maintainer
model: human
release: M1
depends_on: [0007, 0009, 0010, 0011]
change: null
---

## Goal
Deliver the first complete developer-facing path from journal initialization through fixture ingestion, explanation, and portable export.

## Acceptance criteria
- [ ] The init command, fixture ingestion, deterministic explain, and versioned JSONL export work end to end.
- [ ] Live collectors are refused in fixture-only mode.

## Context
This slice proves the core data path before any ambient collection is allowed. Its outputs become the basis for compatibility tests.

## Notes
The 0.0.1 demo, schema, fixture export, and capture-refusal commands prove most of the fixture path. A production-style init command and the unfinished writer/recovery dependencies keep this issue open.
