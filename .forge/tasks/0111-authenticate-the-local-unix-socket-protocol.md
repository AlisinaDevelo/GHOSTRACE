---
id: 0111
title: Authenticate the local Unix-socket protocol
status: backlog
agent: security-auditor
model: human
release: M5
parent: 0035
depends_on: [0010, 0018, 0020, 0029]
change: null
workstream: service-ui
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Expose a versioned local service only through a restrictive Unix socket with verified peer, instance, request, and capability context.

## Acceptance criteria
- [ ] Socket directory and file ownership and mode are verified without following links, and no TCP listener is created.
- [ ] Peer credentials, service instance, protocol version, request size, deadlines, and replay semantics are validated before dispatch.
- [ ] Read, export, policy, lifecycle, and administrative capabilities are separate and denied by default.

## Context
Filesystem permissions alone do not define the complete local-service authorization contract.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
