---
id: 0110
title: Run a Safari WebExtension parity gate
status: backlog
agent: macos-engineer
model: human
release: M5
parent: 0034
depends_on: [0029, 0108, 0109]
change: null
workstream: browser
type: spike
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Decide whether Safari can satisfy the same permission, pairing, private-context, minimization, lifecycle, and update contracts without weakening the Chromium baseline.

## Acceptance criteria
- [ ] A native prototype and ADR map Safari app-extension constraints to every browser security requirement.
- [ ] Permission prompts, App Group or messaging boundary, signing, packaging, review, private mode, and update behavior are tested where authorized.
- [ ] The decision ships a bounded adapter or records a no-go with evidence and no fallback network listener.

## Context
Platform parity is optional; the privacy contract is not.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
