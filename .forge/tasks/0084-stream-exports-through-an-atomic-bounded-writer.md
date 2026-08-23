---
id: 0084
title: Stream exports through an atomic bounded writer
status: backlog
agent: implementation-engineer
model: human
release: M3
parent: 0020
depends_on: [0083]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Export arbitrarily large authorized journals without loading all plaintext into memory or leaving a partial destination presented as complete.

## Acceptance criteria
- [ ] Records stream in stable order with bounded buffers and incremental manifest digests.
- [ ] Temporary creation, permissions, fsync, rename, cancellation, disk-full, and existing-destination behavior are fault-tested.
- [ ] Partial output is either removed or unmistakably marked incomplete and never carries a valid final manifest.

## Context
The fixture path is bounded, but a live journal requires streaming and crash-safe plaintext handling.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
