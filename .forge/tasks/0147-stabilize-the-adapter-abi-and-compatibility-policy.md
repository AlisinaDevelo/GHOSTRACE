---
id: 0147
title: Stabilize the adapter ABI and compatibility policy
status: backlog
agent: api-designer
model: human
release: M10
parent: 0146
depends_on: [0136, 0137, 0139]
change: null
workstream: ecosystem
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Publish a minimal versioned adapter boundary with explicit ownership, memory, cancellation, threading, event, gap, error, and capability semantics.

## Acceptance criteria
- [ ] The ABI or protocol has language-neutral fixtures, generated bindings where applicable, and compatibility tests across supported versions.
- [ ] Breaking, additive, deprecated, experimental, and security-revoked changes have distinct version and negotiation behavior.
- [ ] Unknown or incompatible adapters refuse before loading or receiving journal capabilities.

## Context
A Rust API alone is not a durable third-party binary contract.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
