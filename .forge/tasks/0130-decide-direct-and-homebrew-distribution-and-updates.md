---
id: 0130
title: Decide direct and Homebrew distribution and updates
status: backlog
agent: release-engineer
model: human
release: M7
parent: 0125
depends_on: [0038, 0040]
change: null
workstream: operations
type: spike
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Choose verifiable distribution and update channels that preserve signing, provenance, rollback, user control, and the no-silent-network baseline.

## Acceptance criteria
- [ ] The ADR compares manual signed downloads, Homebrew cask or formula, Sparkle-style updates, and no automatic updater.
- [ ] Each accepted channel verifies artifact digest, signature, provenance, version monotonicity, downgrade policy, and consent before installation.
- [ ] Update checks are disabled by default unless the product contract and network inventory are explicitly revised and tested.

## Context
Convenient updates create a new network and supply-chain boundary and therefore require a separate decision.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
