---
id: 0056
title: Test locked-session and background key behavior
status: backlog
agent: test-engineer
model: human
release: M1
parent: 0008
depends_on: [0054]
change: null
workstream: storage
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Measure and define journal behavior across login, lock, sleep, wake, fast-user-switch, logout, and launchd restart conditions.

## Acceptance criteria
- [ ] A macOS integration matrix records Keychain availability and prompts for every lifecycle transition.
- [ ] Collectors buffer only within an explicit bound or emit a gap when the key is unavailable.
- [ ] No fallback key, plaintext queue, or silent data loss is permitted.

## Context
Keychain accessibility and launch context determine whether a background collector can safely persist events.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
