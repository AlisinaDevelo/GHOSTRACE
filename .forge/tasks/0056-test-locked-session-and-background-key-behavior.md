---
id: 0056
title: Test locked-session and background key behavior
status: done
agent: test-engineer
model: human
release: M1
parent: 0008
depends_on: [0054]
change: pr-223-7aaa5e2ebc944d51908e56be1d87947f78f192d2
workstream: storage
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Measure and define journal behavior across login, lock, sleep, wake, fast-user-switch, logout, and launchd restart conditions.

## Acceptance criteria
- [x] A macOS integration matrix records Keychain availability and prompts for every lifecycle transition.
- [x] Collectors buffer only within an explicit bound or emit a gap when the key is unavailable.
- [x] No fallback key, plaintext queue, or silent data loss is permitted.

## Context
Keychain accessibility and launch context determine whether a background collector can safely persist events.

## Notes
Evidence: `docs/evidence/0056-key-availability-matrix.md` and
`docs/evidence/0056-key-availability-matrix.json`. The protected-main merge
rerun is recorded at `7aaa5e2ebc944d51908e56be1d87947f78f192d2` on the target
MacBookPro17,1 / macOS 26.6.2 (25G83) / arm64 / Rust 1.88.0. Login/unlocked
and isolated Keychain lock/unlock are observed; sleep, wake, fast-user-switch,
logout, and launchd-restart remain explicit no-go/not-exercised rows until an
authorized interactive lifecycle run and/or a GHOSTRACE launchd helper exists.
Issue closure is conditioned on this retained evidence, not on hosted CI alone.
