---
id: 0084
title: Stream exports through an atomic bounded writer
status: done
agent: implementation-engineer
model: human
release: M3
parent: 0020
depends_on: [0083]
change: pr-291
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Export arbitrarily large authorized journals without loading all plaintext into memory or leaving a partial destination presented as complete.

## Acceptance criteria
- [x] Records stream in stable order with bounded buffers and incremental manifest digests.
- [x] Temporary creation, permissions, fsync, rename, cancellation, disk-full, and existing-destination behavior are fault-tested.
- [x] Partial output is either removed or unmistakably marked incomplete and never carries a valid final manifest.

## Context
The fixture path is bounded, but a live journal requires streaming and crash-safe plaintext handling.

## Notes
Implemented in PR #291 and reproduced on protected `main`; retained proof is [docs/evidence/0084-stream-exports.md](../../docs/evidence/0084-stream-exports.md). Completion requires the acceptance evidence above; issue closure alone is not evidence.
