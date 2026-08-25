---
id: 0012
title: Ship fixture ingest, explain, and JSONL export CLI slice
status: done
agent: maintainer
model: human
release: M1
depends_on: [0007, 0009, 0010, 0011]
change: pr-235-30172a2a51ddaadea8dcdf7cc6d0d2d8361d6ec0
workstream: foundation
type: feature
priority: p0
risks: []
platform: any
---

## Goal
Deliver the first complete developer-facing path from journal initialization through fixture ingestion, explanation, and portable export.

## Acceptance criteria
- [x] The init command, fixture ingestion, deterministic explain, and versioned JSONL export work end to end.
- [x] Live collectors are refused in fixture-only mode.

## Context
This slice proves the core data path before any ambient collection is allowed. Its outputs become the basis for compatibility tests.

## Notes
The durable `init`, `ingest`, `explain`, and `export --journal` commands now reopen the
same hardened SQLite journal across processes. The fixture-only CLI uses a deterministic
synthetic key for this headstart; production Keychain-backed live capture remains gated.
The implementation, device receipts, and scope limits are recorded in
`docs/evidence/0012-cli-vertical-slice.md`.
