---
id: 0016
title: Add event-storm backpressure and loss accounting
status: done
agent: maintainer
model: human
release: M2
depends_on: [0010, 0013, 0015, 0073, 0074]
change: pr-272
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Keep collection stable under bursts and make every forced loss measurable and visible in the journal.

## Acceptance criteria
- [x] Memory remains bounded under an event storm.
- [x] Sustained synthetic load is measured.
- [x] Induced drops create an auditable gap and collector status.

## Context
Backpressure behavior must be intentional across collector queues, policy evaluation, encryption, and the single database writer.

## Notes
Implementation PR #272 (`cdbf5979110113bd9b3257cbb9d1580f45f271d2`) is merged
to protected `main` at `954ec4fb9966f36906db65b141f90dbe0f6d4790`. The
implementation adds a bounded callback queue cap, cumulative overflow/status
counters, durable loss gaps, and one bounded status-admission reservation.
Debug and release synthetic stress were rerun from that exact protected SHA on
the named macOS arm64 device. Retained acceptance evidence is
`docs/evidence/0016-event-storm-backpressure.md`; the public issue remains open
until the evidence change is merged and linked.
