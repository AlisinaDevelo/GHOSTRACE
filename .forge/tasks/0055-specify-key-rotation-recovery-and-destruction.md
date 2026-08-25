---
id: 0055
title: Specify key rotation, recovery, and destruction
status: done
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
- [x] The envelope records key generation and algorithm without exposing key material.
- [x] Rotation is resumable and old ciphertext remains readable until a verified commit retires the prior key.
- [x] Lost-key and reset flows state exactly which data becomes unrecoverable and require explicit confirmation.

## Context
Key lifecycle errors can destroy availability or silently weaken confidentiality unless modeled before production storage.

## Notes
Implemented in `src/crypto.rs` and `src/key_lifecycle.rs`, with strict schema and
integration coverage in `tests/key_lifecycle.rs`. Completion requires the device
receipts in `docs/evidence/0055-key-rotation-recovery.md`; issue closure alone is not
evidence.
