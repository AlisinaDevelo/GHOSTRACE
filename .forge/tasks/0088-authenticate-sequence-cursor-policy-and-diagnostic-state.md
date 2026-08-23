---
id: 0088
title: Authenticate sequence, cursor, policy, and diagnostic state
status: backlog
agent: security-auditor
model: human
release: M3
parent: 0023
depends_on: [0008, 0009, 0020]
change: null
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Extend tamper evidence beyond payload metadata to ordering, recovery cursors, policy history, coverage gaps, and diagnostic transitions.

## Acceptance criteria
- [ ] The authenticated structure defines canonical bytes, domain separation, chain boundaries, and deletion semantics.
- [ ] Edits, insertion, deletion, reorder, truncation, cursor rollback, and policy substitution are detected by the verifier.
- [ ] Verification never claims origin authenticity beyond the local key and documented threat model.

## Context
Unauthenticated journal state can change the story even when individual payload decryption succeeds.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
