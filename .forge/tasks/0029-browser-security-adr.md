---
id: 0029
title: Decide browser transport and permissions in security ADR
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0002, 0004, 0005, 0012, 0101]
change: null
workstream: browser
type: spike
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Freeze the local browser trust boundary, minimum permissions, transport, pairing, and private-context rules before extension code is shipped.

## Acceptance criteria
- [ ] The ADR records the Native Messaging and Unix-socket transport choice.
- [ ] Extension allowlist, pairing, message limits, and minimum permissions are documented.
- [ ] Private-context policy is documented.
- [ ] Localhost HTTP is explicitly rejected.

## Context
Browser data is attacker-shaped and frequently contains secrets. The design must minimize permissions, input size, retained fields, and local attack surface.

## Notes
No implementation notes yet.
