---
id: 0102
title: Install and remove the native-host manifest safely
status: backlog
agent: macos-engineer
model: human
release: M5
parent: 0030
depends_on: [0008, 0010, 0029]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Manage browser native-messaging registration as an explicit, reversible, verified local mutation for each supported browser channel.

## Acceptance criteria
- [ ] Plan, install, verify, upgrade, disable, and uninstall preserve unrelated manifests and reject unsafe ownership, modes, links, and paths.
- [ ] Allowed extension origins are exact identifiers and wildcard or unknown origins are refused.
- [ ] Uninstall removes only a manifest whose digest and installation receipt still match.

## Context
Chrome native messaging launches a registered host over stdio and relies on an allowed-origins manifest.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
