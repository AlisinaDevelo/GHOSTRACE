---
id: 0028
title: Add frontmost attribution and privacy tests
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0027, 0018]
change: null
---

## Goal
Measure whether application transitions remain accurate, ordered, bounded, and privacy-safe during real lifecycle edge cases.

## Acceptance criteria
- [ ] Rapid application switches and application termination are tested.
- [ ] Unknown applications and lifecycle gaps are tested.
- [ ] Deterministic ordering and attribution latency are measured.

## Context
Tests should demonstrate both useful attribution and the absence of window titles, document names, and privileged capture fields.

## Notes
No implementation notes yet.
