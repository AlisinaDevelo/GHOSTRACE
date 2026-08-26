---
id: 0085
title: Add export redaction preview and policy receipts
status: done
agent: privacy-engineer
model: human
release: M3
parent: 0020
depends_on: [0007, 0083]
change: pr-293
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Show exactly which sources, fields, time ranges, profiles, gaps, and record counts will leave the encrypted journal before writing plaintext.

## Acceptance criteria
- [x] Preview and execution use one immutable query and redaction plan digest.
- [x] The preview warns that plaintext metadata will leave encrypted storage and requires an explicit confirmation bound to that plan digest.
- [x] A policy change or journal snapshot change between preview and execution requires reconfirmation or produces a clearly bounded delta.
- [x] The receipt records destination class and manifest digest without retaining the destination path in normal diagnostics.

## Context
Explicit export is a declassification action and deserves a reviewable plan.

## Notes
Implemented in PR #293 and merged to protected `main` at `26a7ade9c039d3fec35079c73acd17d9540b49e9`; retained proof is [docs/evidence/0085-export-redaction-preview.md](../../docs/evidence/0085-export-redaction-preview.md). Completion requires the acceptance evidence above; issue closure alone is not evidence.
