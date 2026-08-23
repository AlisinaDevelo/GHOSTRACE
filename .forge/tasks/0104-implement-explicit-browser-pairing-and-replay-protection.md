---
id: 0104
title: Implement explicit browser pairing and replay protection
status: backlog
agent: security-auditor
model: human
release: M5
parent: 0030
depends_on: [0102]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Bind a user-approved extension installation and native host to a revocable local channel without treating manifest presence as sufficient consent.

## Acceptance criteria
- [ ] Pairing shows browser, profile class, extension identity, requested event classes, retained fields, and private-context policy.
- [ ] Session and message nonces, expiry, monotonic sequence, restart, revocation, and extension-update behavior are specified and tested.
- [ ] Copied manifests, replaced extensions, replayed transcripts, and stale approvals require refusal or re-pairing.

## Context
Registration proves browser authorization to launch a host, not the user's current consent to retain events.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
