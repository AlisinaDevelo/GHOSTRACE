---
id: 0022
title: Add optional Parquet cold-archive export
status: done
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
- [x] Parquet schema metadata and checksums are documented.
- [x] JSONL and Parquet representations compare successfully.
- [x] Archive creation is explicit and never automatic.
- [x] The command warns about plaintext disclosure.

## Context
Parquet is a derived export, not the canonical store. Compatibility and checksums must make conversion errors detectable.

## Notes
The writer ships behind an opt-in `parquet` cargo feature using the `parquet` crate without arrow, so default builds and the reviewed release binary do not change.
No implementation notes yet.
