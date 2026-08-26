---
id: 0088
title: Authenticate sequence, cursor, policy, and diagnostic state
status: done
agent: security-auditor
model: human
release: M3
parent: 0023
depends_on: [0008, 0009, 0020]
change: pr-308
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Extend tamper evidence beyond payload metadata to ordering, recovery cursors, policy history, coverage gaps, and diagnostic transitions.

## Acceptance criteria
- [x] The authenticated structure defines canonical bytes, domain separation, chain boundaries, and deletion semantics.
- [x] Edits, insertion, deletion, reorder, truncation, cursor rollback, and policy substitution are detected by the verifier.
- [x] Verification never claims origin authenticity beyond the local key and documented threat model.

## Context
Unauthenticated journal state can change the story even when individual payload decryption succeeds.

## Notes
Implemented in PR #308 and reproduced on protected-main merge
`ee25edd185dce500a4df06856b8b2ecbba67ee3d`. Completion evidence is retained in
`docs/evidence/0088-authenticated-state.md`; issue closure alone is not evidence.
