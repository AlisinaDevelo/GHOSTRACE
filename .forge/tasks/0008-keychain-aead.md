---
id: 0008
title: Implement Keychain-backed DEK and AEAD envelopes
status: review
agent: maintainer
model: human
release: M1
depends_on: [0003, 0004, 0054, 0055, 0056]
change: pr-225-a954aab
workstream: storage
type: feature
priority: p0
risks: [security]
platform: macos
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
The encrypted journal and Keychain provider landed in protected-main merges
#219, #223, and #224. This review adds a focused regression suite for the
fail-closed, pre-insert, ciphertext, metadata, and redaction boundaries. The
isolated Keychain lifecycle probe observes login/unlocked and lock/unlock; it
does not claim support for interactive sleep, wake, fast-user-switch, logout,
or launchd restart. Protected-main rerun evidence will be added before the
task is closed.
