---
id: 0060
title: Specify bounded writer queue and acknowledgement semantics
status: review
agent: concurrency-specialist
model: human
release: M1
parent: 0010
depends_on: [0049, 0051, 0057]
change: null
workstream: storage
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Define admission, ordering, cancellation, backpressure, transaction, and acknowledgement behavior for the single durable writer.

## Acceptance criteria
- [ ] Queue item, batch, memory, wait-time, and retry bounds are configuration contracts with safe defaults.
- [ ] An acknowledgement is emitted only after event, cursor, policy reference, and diagnostics commit atomically.
- [ ] Full queues block, reject, or emit a gap according to a tested source-specific policy and never drop silently.

## Context
Writer semantics are the point where source coverage becomes durable evidence or an explicit gap.

## Notes
Implementation is ready for review in the bounded writer and journal transaction
paths. Completion requires the protected-main device rerun and retained evidence;
issue closure or hosted CI alone is not evidence.
