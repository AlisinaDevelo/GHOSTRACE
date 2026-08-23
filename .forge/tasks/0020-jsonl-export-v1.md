---
id: 0020
title: Define and ship JSONL export v1 with manifest
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0018, 0019, 0083, 0084, 0085]
change: null
workstream: explain-export
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Ship a documented, streaming, versioned export that carries enough context to interpret journal records outside the application.

## Acceptance criteria
- [ ] The manifest includes version, policy, coverage, collector status, and gaps.
- [ ] A 10-million-record fixture export completes through a bounded streaming buffer without materializing the result and with no more than 64 MiB peak incremental resident memory.
- [ ] Before writing plaintext, the command identifies the selected sources and time range and requires explicit confirmation that sensitive metadata may leave encrypted storage.
- [ ] Existing destinations are not overwritten without explicit confirmation.

## Context
JSONL is the first portable compatibility boundary. Exports are user initiated and must warn when sensitive plaintext may leave encrypted storage.

## Notes
The bounded fixture exporter atomically writes a versioned manifest, policy ID, coverage, collector status, and gap records, and refuses overwrite without --force. Streaming large-journal behavior and dependent query/explain gates remain open.
