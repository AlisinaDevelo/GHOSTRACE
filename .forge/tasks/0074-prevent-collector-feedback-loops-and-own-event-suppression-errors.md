---
id: 0074
title: Prevent collector feedback loops and own-event suppression errors
status: done
agent: security-auditor
model: human
release: M2
parent: 0016
depends_on: [0065, 0069, 0070]
change: pr-265
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Keep journal, export, backup, and temporary-file writes from recursively generating misleading evidence without hiding unrelated changes.

## Acceptance criteria
- [x] Internal storage paths are denied before persistence and tested across relocation and symlink attempts.
- [x] OwnEvent is treated as source evidence rather than an unconditional drop rule.
- [x] Concurrent external writes under internal-looking paths remain denied or surfaced according to the documented policy.

## Context
Naive suppression can create both event storms and invisible attacker-controlled changes.

## Notes
Implemented in PR #265 and merged to protected `main` at
`46ad1776da9369cd9401f1276371a8e5373826b3`. Completion evidence is retained in
`docs/evidence/0074-collector-feedback-loop-policy.md`; issue closure alone is
not evidence.
