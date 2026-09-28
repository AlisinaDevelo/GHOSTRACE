---
id: 0161
title: Ship an explicit ghostrace run CLI over a Keychain journal
status: backlog
agent: macos-engineer
model: human
release: M4
parent: 0024
depends_on: [0024, 0054, 0164, 0165]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Expose the consent-gated shell wrapper as `ghostrace run -- <program> [args...]` so a person can record one deliberately wrapped command into a durable, Keychain-encrypted journal without writing Rust.

## Acceptance criteria
- [ ] The command refuses to run until a persisted consent receipt for a shell-enabled policy exists, and `ghostrace run` never becomes ambient capture.
- [ ] The durable journal uses Keychain key custody (data-protection when signed, or the explicit login-keychain option from 0164); the fixture seed is never used for a live journal.
- [ ] The command exits with the child's status, prints nothing derived from arguments, environment, or terminal streams, and a revoked consent refuses before spawning.

## Context
The library adapter ships first so the privacy boundary is testable in isolation. A CLI adds persisted consent, key custody, and journal-path handling that need their own review.

## Notes
Follow-up to 0024. Completion requires the acceptance evidence above; issue closure alone is not evidence.
