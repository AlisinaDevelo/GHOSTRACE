---
id: 0138
title: Design encrypted evidence bundle transfer
status: backlog
agent: security-auditor
model: human
release: M8
parent: 0132
depends_on: [0133, 0134]
change: null
workstream: interoperability
type: feature
priority: p2
risks: [privacy, security]
platform: any
---

## Goal
Allow an explicit offline export for a named recipient while keeping transport, recipient verification, key handling, expiry, and disclosure outside the ambient product path.

## Acceptance criteria
- [ ] The bundle binds manifest, recipient key identity, algorithms, sender tool version, policy profile, expiry, and plaintext digest commitments.
- [ ] Preview and confirmation show exactly which minimized evidence leaves the journal and warn that recipient deletion is not enforceable.
- [ ] Wrong recipient, tamper, truncation, replay, expired key, key rotation, and interrupted creation or import are tested.

## Context
Portable encrypted bundles can support collaboration without introducing a hosted sync service.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
