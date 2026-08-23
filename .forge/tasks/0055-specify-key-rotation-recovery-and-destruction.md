---
id: 0055
title: Specify key rotation, recovery, and destruction
status: backlog
agent: security-auditor
model: human
release: M1
parent: 0008
depends_on: [0054]
change: null
workstream: storage
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Provide crash-safe key rotation and explicit loss, compromise, reset, and destruction procedures without introducing a cloud recovery secret.

## Acceptance criteria
- [ ] The envelope records key generation and algorithm without exposing key material.
- [ ] Rotation is resumable and old ciphertext remains readable until a verified commit retires the prior key.
- [ ] Lost-key and reset flows state exactly which data becomes unrecoverable and require explicit confirmation.

## Context
Key lifecycle errors can destroy availability or silently weaken confidentiality unless modeled before production storage.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
