---
id: 0016
title: Add event-storm backpressure and loss accounting
status: backlog
agent: maintainer
model: human
release: M2
depends_on: [0010, 0013, 0015]
change: null
---

## Goal
Keep collection stable under bursts and make every forced loss measurable and visible in the journal.

## Acceptance criteria
- [ ] Memory remains bounded under an event storm.
- [ ] Sustained synthetic load is measured.
- [ ] Induced drops create an auditable gap and collector status.

## Context
Backpressure behavior must be intentional across collector queues, policy evaluation, encryption, and the single database writer.

## Notes
No implementation notes yet.
