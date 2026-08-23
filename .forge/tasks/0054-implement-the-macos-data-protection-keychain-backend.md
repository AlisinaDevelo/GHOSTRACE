---
id: 0054
title: Implement the macOS data-protection Keychain backend
status: ready
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
- [ ] SecItem operations set the data-protection Keychain flag and disable iCloud synchronization.
- [ ] Bundle, access-group, login-session, and command-line helper constraints are documented and integration-tested on macOS.
- [ ] Missing, duplicated, inaccessible, or malformed key items fail closed with redacted errors.

## Context
Apple recommends the data-protection Keychain for modern SecItem use, but it is available only in a user login context.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
