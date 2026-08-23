---
id: 0022
title: Add optional Parquet cold-archive export
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0020, 0021, 0090]
change: null
workstream: explain-export
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Add an explicit columnar archive format for long-term analysis without changing the live journal or silently deleting source records.

## Acceptance criteria
- [ ] Parquet schema metadata and checksums are documented.
- [ ] JSONL and Parquet representations compare successfully.
- [ ] Archive creation is explicit and never automatic.
- [ ] The command warns about plaintext disclosure.

## Context
Parquet is a derived export, not the canonical store. Compatibility and checksums must make conversion errors detectable.

## Notes
No implementation notes yet.
