---
id: 0010
title: Build bounded ingest writer with atomic cursor commit
status: done
agent: maintainer
model: human
release: M1
depends_on: [0006, 0007, 0008, 0009, 0049, 0060, 0061]
change: pr-231-632f6b255bcea3923a7f461ba1d686beb15307d2
workstream: storage
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Implement the durable ingestion boundary so accepted events and collector positions advance together without silent loss.

## Acceptance criteria
- [x] Ingestion uses one writer and a bounded queue.
- [x] Acknowledgement occurs only after commit.
- [x] Each event and its collector cursor commit transactionally.
- [x] Queue saturation is observable.

## Context
A source must be able to replay anything not committed after a crash. Capacity pressure must become status or an explicit gap, never an invisible drop.

## Notes
Fixture batches serialize through one connection and commit event rows with cursor updates transactionally. The bounded asynchronous queue, post-commit acknowledgement contract, saturation outcomes, and crash/retry evidence are implemented and retained in `docs/evidence/0010-bounded-ingest-writer.md`. The verified scope remains fixture-only; live collectors and production throughput characterization are separate gates.
