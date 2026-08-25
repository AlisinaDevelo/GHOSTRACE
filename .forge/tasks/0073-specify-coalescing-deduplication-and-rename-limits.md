---
id: 0073
title: Specify coalescing, deduplication, and rename limits
status: done
agent: architect
model: human
release: M2
parent: 0016
depends_on: [0065, 0070, 0071]
change: pr-262-47ed8164d55462befe8e0b8194245dd2b4bfa516
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Preserve FSEvents coalescing and rename ambiguity while preventing exact duplicate deliveries from inflating explanations.

## Acceptance criteria
- [x] The contract distinguishes source coalescing, transport duplication, repeated modification, and inferred rename pairs.
- [x] Deduplication keys and time bounds are deterministic and never erase distinct source evidence.
- [x] Rename explanations state when old-to-new pairing is unknown or only contextual.

## Context
Temporal proximity is not proof that two rename-shaped notifications are the same object.

## Notes
Implemented in [PR #262](https://github.com/AlisinaDevelo/GHOSTRACE/pull/262),
merged to protected `main` at
`47ed8164d55462befe8e0b8194245dd2b4bfa516`. The complete protected-main
receipt is retained in
[`docs/evidence/0073-fsevents-observation-contract.md`](../../docs/evidence/0073-fsevents-observation-contract.md).
