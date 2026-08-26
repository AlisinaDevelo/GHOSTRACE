---
id: 0020
title: Define and ship JSONL export v1 with manifest
status: done
agent: maintainer
model: human
release: M3
depends_on: [0018, 0019, 0083, 0084, 0085]
change: pr-300
workstream: explain-export
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Ship a documented, streaming, versioned export that carries enough context to interpret journal records outside the application.

## Acceptance criteria
- [x] The manifest includes version, policy, coverage, collector status, and gaps.
- [x] A 10-million-record fixture export completes through a bounded streaming buffer without materializing the result and with no more than 64 MiB peak incremental resident memory.
- [x] Before writing plaintext, the command identifies the selected sources and time range and requires explicit confirmation that sensitive metadata may leave encrypted storage.
- [x] Existing destinations are not overwritten without explicit confirmation.

## Context
JSONL is the first portable compatibility boundary. Exports are user initiated and must warn when sensitive plaintext may leave encrypted storage.

## Notes
The parent capability is complete through the schema registry, atomic bounded
writer, redaction preview, and ten-million-record scale lane in PR #300. The
retained acceptance record is `docs/evidence/0020-jsonl-export-v1.md`; issue
closure follows that evidence merge and independent live verification.
