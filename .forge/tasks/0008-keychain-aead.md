---
id: 0008
title: Implement Keychain-backed DEK and AEAD envelopes
status: ready
agent: maintainer
model: human
release: M1
depends_on: [0003, 0004]
change: null
---

## Goal
Protect sensitive event payloads at rest with authenticated encryption and a macOS Keychain-backed data-encryption key.

## Acceptance criteria
- [ ] Payloads are encrypted before SQLite insertion.
- [ ] Missing keys fail closed.
- [ ] Keys never appear in logs or environment variables.
- [ ] macOS Keychain integration and a deterministic test provider work.

## Context
The storage design may expose limited metadata such as timestamps and event kinds; that residual leakage must be documented.

## Notes
The fixture journal encrypts payloads with XChaCha20-Poly1305, authenticates plaintext event metadata as associated data, and uses a deterministic test provider. No production key provider or macOS Keychain integration is shipped, so this task remains open.
