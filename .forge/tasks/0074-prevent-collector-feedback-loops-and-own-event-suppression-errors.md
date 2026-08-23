---
id: 0074
title: Prevent collector feedback loops and own-event suppression errors
status: backlog
agent: security-auditor
model: human
release: M2
parent: 0016
depends_on: [0065, 0069, 0070]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Keep journal, export, backup, and temporary-file writes from recursively generating misleading evidence without hiding unrelated changes.

## Acceptance criteria
- [ ] Internal storage paths are denied before persistence and tested across relocation and symlink attempts.
- [ ] OwnEvent is treated as source evidence rather than an unconditional drop rule.
- [ ] Concurrent external writes under internal-looking paths remain denied or surfaced according to the documented policy.

## Context
Naive suppression can create both event storms and invisible attacker-controlled changes.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
