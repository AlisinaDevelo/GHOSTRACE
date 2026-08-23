---
id: 0155
title: Implement cryptographic agility and key migration
status: backlog
agent: cryptography-specialist
model: human
release: M11
parent: 0153
depends_on: [0055, 0089, 0145]
change: null
workstream: long-term
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Support reviewed algorithm, parameter, envelope, and Keychain changes without silent downgrade, nonce reuse, key confusion, or loss of historical verification.

## Acceptance criteria
- [ ] A versioned algorithm registry defines permitted, deprecated, forbidden, and migration-only suites with domain separation.
- [ ] Migration is resumable, checkpointed, power-failure tested, rollback-safe until commit, and produces before-and-after verification manifests.
- [ ] Unknown suites, missing keys, partial rotation, downgrade, altered parameters, and mixed generations fail with explicit recovery options.

## Context
Long-lived encrypted journals must outlast the initial algorithm choice without weakening old evidence.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
