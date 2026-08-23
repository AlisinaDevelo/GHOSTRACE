---
id: 0010
title: Build bounded ingest writer with atomic cursor commit
status: backlog
agent: maintainer
model: human
release: M1
depends_on: [0006, 0007, 0008, 0009]
change: null
---

## Goal
Implement the durable ingestion boundary so accepted events and collector positions advance together without silent loss.

## Acceptance criteria
- [ ] Ingestion uses one writer and a bounded queue.
- [ ] Acknowledgement occurs only after commit.
- [ ] Each event and its collector cursor commit transactionally.
- [ ] Queue saturation is observable.

## Context
A source must be able to replay anything not committed after a crash. Capacity pressure must become status or an explicit gap, never an invisible drop.

## Notes
Fixture batches serialize through one connection and commit event rows with cursor updates transactionally. A bounded asynchronous queue, post-commit acknowledgement contract, saturation telemetry, and crash evidence remain open.
