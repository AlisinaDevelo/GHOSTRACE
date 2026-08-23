---
id: 0013
title: Implement selected-root macOS FSEvents collector
status: backlog
agent: maintainer
model: human
release: M2
depends_on: [0002, 0007, 0010, 0012]
change: null
---

## Goal
Add the first live source while limiting observation to filesystem metadata under roots the user explicitly selects.

## Acceptance criteria
- [ ] Root collection requires explicit opt-in.
- [ ] File-level events are captured without content access.
- [ ] Collector lifecycle status is visible.
- [ ] Controlled create, modify, move, and delete integration tests pass.

## Context
FSEvents reports changes rather than a complete causal record. The collector must expose that limitation and retain source flags without overstating certainty.

## Notes
No implementation notes yet.
