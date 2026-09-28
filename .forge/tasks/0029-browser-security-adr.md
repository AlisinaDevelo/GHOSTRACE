---
id: 0029
title: Decide browser transport and permissions in security ADR
status: done
agent: maintainer
model: human
release: M5
depends_on: [0002, 0004, 0005, 0012, 0101]
change: pr-371
workstream: browser
type: spike
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Freeze the local browser trust boundary, minimum permissions, transport, pairing, and private-context rules before extension code is shipped.

## Acceptance criteria
- [x] The ADR records the Native Messaging and Unix-socket transport choice.
- [x] Extension allowlist, pairing, message limits, and minimum permissions are documented.
- [x] Private-context policy is documented.
- [x] Localhost HTTP is explicitly rejected.

## Context
Browser data is attacker-shaped and frequently contains secrets. The design must minimize permissions, input size, retained fields, and local attack surface.

## Notes
Implemented in PR #371 and squash-merged to protected `main` at
`408ddf0cbf0dccaf14b087ae79aaa4c7a0ac6081`. Verified on merged `main`; see
[`docs/evidence/0029-browser-transport-adr.md`](../../docs/evidence/0029-browser-transport-adr.md)
for command-level device receipts and limitations.
