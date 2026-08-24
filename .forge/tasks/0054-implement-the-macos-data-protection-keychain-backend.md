---
id: 0054
title: Implement the macOS data-protection Keychain backend
status: done
agent: macos-engineer
model: human
release: M1
parent: 0008
depends_on: [0003, 0004]
change: null
workstream: storage
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Store only the journal wrapping key in the per-user macOS data-protection Keychain with explicit non-synchronizing access controls.

## Acceptance criteria
- [x] SecItem operations set the data-protection Keychain flag and disable iCloud synchronization.
- [x] Bundle, access-group, login-session, and command-line helper constraints are documented and integration-tested on macOS.
- [x] Missing, duplicated, inaccessible, or malformed key items fail closed with redacted errors.

## Context
Apple recommends the data-protection Keychain for modern SecItem use, but it is available only in a user login context.

## Notes
Implemented in `1137d7e79e8ef04f09d76ef9c347fdbacd526ab1` and verified in
`docs/evidence/0054-keychain-backend.md`. The local macOS run uses an unsigned CLI,
so the integration test records the bounded inaccessible-item refusal path rather
than claiming a signed-helper data-protection round trip; no legacy-keychain
fallback is permitted.
