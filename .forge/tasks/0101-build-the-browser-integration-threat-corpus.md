---
id: 0101
title: Build the browser integration threat corpus
status: backlog
agent: security-auditor
model: human
release: M5
parent: 0029
depends_on: [0002, 0004, 0005, 0012]
change: null
workstream: browser
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Exercise the proposed browser boundary against hostile pages, compromised extensions, malformed native messages, permission drift, and private-context mistakes before implementation ships.

## Acceptance criteria
- [ ] The corpus covers spoofed origins, oversized frames, duplicate and replayed messages, Unicode and URL confusion, extension replacement, and downgrade attempts.
- [ ] Every case states which layer validates, rejects, records a gap, or requires re-pairing.
- [ ] The security ADR links each accepted risk to a test, permission, user control, and rollback path.

## Context
Browser messages are attacker-shaped and cross a privileged native boundary.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
