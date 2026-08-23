---
id: 0073
title: Specify coalescing, deduplication, and rename limits
status: backlog
agent: architect
model: human
release: M2
parent: 0016
depends_on: [0065, 0070, 0071]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Preserve FSEvents coalescing and rename ambiguity while preventing exact duplicate deliveries from inflating explanations.

## Acceptance criteria
- [ ] The contract distinguishes source coalescing, transport duplication, repeated modification, and inferred rename pairs.
- [ ] Deduplication keys and time bounds are deterministic and never erase distinct source evidence.
- [ ] Rename explanations state when old-to-new pairing is unknown or only contextual.

## Context
Temporal proximity is not proof that two rename-shaped notifications are the same object.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
