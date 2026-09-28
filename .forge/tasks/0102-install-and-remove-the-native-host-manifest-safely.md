---
id: 0102
title: Install and remove the native-host manifest safely
status: done
agent: macos-engineer
model: human
release: M5
parent: 0030
depends_on: [0008, 0010, 0029]
change: pr-374
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Manage browser native-messaging registration as an explicit, reversible, verified local mutation for each supported browser channel.

## Acceptance criteria
- [x] Plan, install, verify, upgrade, disable, and uninstall preserve unrelated manifests and reject unsafe ownership, modes, links, and paths.
- [x] Allowed extension origins are exact identifiers and wildcard or unknown origins are refused.
- [x] Uninstall removes only a manifest whose digest and installation receipt still match.

## Context
Chrome native messaging launches a registered host over stdio and relies on an allowed-origins manifest.

## Notes
Implemented in PR #374 and squash-merged to protected `main` at
`8652853d3425060ce1f31750ac115171482412fa`. Verified on merged `main` at `37ff5103ff69193db1f284967481db594b51ac90`; see
[`docs/evidence/0102-native-host-manifest.md`](../../docs/evidence/0102-native-host-manifest.md)
for command-level device receipts and limitations.
