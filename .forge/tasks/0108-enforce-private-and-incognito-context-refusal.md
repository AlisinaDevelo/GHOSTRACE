---
id: 0108
title: Enforce private and incognito context refusal
status: backlog
agent: privacy-engineer
model: human
release: M5
parent: 0033
depends_on: [0105, 0107]
change: null
workstream: browser
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Prove private contexts are denied before native transport and remain denied after browser, extension, profile, policy, and host restarts.

## Acceptance criteria
- [ ] Incognito-not-allowed is the default manifest and runtime configuration.
- [ ] Split and spanning modes, private windows, guest profiles, ephemeral profiles, and mislabeled messages have tested refusal behavior.
- [ ] No private-context event, count, origin, timing, or error detail is retained except a policy-level bounded denial when explicitly configured.

## Context
Private browsing is a hard product boundary rather than a redaction mode.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
