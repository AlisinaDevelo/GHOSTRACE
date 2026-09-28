---
id: 0101
title: Build the browser integration threat corpus
status: done
agent: security-auditor
model: human
release: M5
parent: 0029
depends_on: [0002, 0004, 0005, 0012]
change: pr-363
workstream: browser
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Exercise the proposed browser boundary against hostile pages, compromised extensions, malformed native messages, permission drift, and private-context mistakes before implementation ships.

## Acceptance criteria
- [x] The corpus covers spoofed origins, oversized frames, duplicate and replayed messages, Unicode and URL confusion, extension replacement, and downgrade attempts.
- [x] Every case states which layer validates, rejects, records a gap, or requires re-pairing.
- [x] The security ADR links each accepted risk to a test, permission, user control, and rollback path.

## Context
Browser messages are attacker-shaped and cross a privileged native boundary.

## Notes
Implemented in PR #363 and squash-merged to protected `main` at
`3c1d035b8451ae08cb429c1d6267e46ca4eccc00`. Verified on merged `main`; see
[`docs/evidence/0101-browser-threat-corpus.md`](../../docs/evidence/0101-browser-threat-corpus.md)
for command-level device receipts and limitations.
