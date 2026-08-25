---
id: 0060
title: Specify bounded writer queue and acknowledgement semantics
status: done
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
- [x] Queue item, batch, memory, wait-time, and retry bounds are configuration contracts with safe defaults.
- [x] An acknowledgement is emitted only after event, cursor, policy reference, and diagnostics commit atomically.
- [x] Full queues block, reject, or emit a gap according to a tested source-specific policy and never drop silently.

## Context
Writer semantics are the point where source coverage becomes durable evidence or an explicit gap.

## Notes
Implemented in PR #203 and merged to protected `main` at
`0c6bae9c5ebf4b7d91ea705fd346fd7c6b541238`. Source and merged-main device
receipts are retained in `docs/evidence/0060-bounded-writer.md`; issue closure or
hosted CI alone is not evidence.
