---
id: 0111
title: Authenticate the local Unix-socket protocol
status: done
agent: security-auditor
model: human
release: M5
parent: 0035
depends_on: [0010, 0018, 0020, 0029]
change: pr-373
workstream: service-ui
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Expose a versioned local service only through a restrictive Unix socket with verified peer, instance, request, and capability context.

## Acceptance criteria
- [x] Socket directory and file ownership and mode are verified without following links, and no TCP listener is created.
- [x] Peer credentials, service instance, protocol version, request size, deadlines, and replay semantics are validated before dispatch.
- [x] Read, export, policy, lifecycle, and administrative capabilities are separate and denied by default.

## Context
Filesystem permissions alone do not define the complete local-service authorization contract.

## Notes
Implemented in PR #373 and squash-merged to protected `main` at
`eee21a5d5f19a5f9e1a3b6b0f33f3ba1f79dccef`. Verified on merged `main` at `37ff5103ff69193db1f284967481db594b51ac90`; see
[`docs/evidence/0111-local-service-socket.md`](../../docs/evidence/0111-local-service-socket.md)
for command-level device receipts and limitations.
