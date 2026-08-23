---
id: 0113
title: Design an accessible evidence and gap interface
status: backlog
agent: accessibility-specialist
model: human
release: M5
parent: 0036
depends_on: [0019, 0020, 0035]
change: null
workstream: service-ui
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Build a read-only timeline and explanation experience that keeps source, evidence level, uncertainty, gaps, policy, and export scope understandable without color-only or hidden semantics.

## Acceptance criteria
- [ ] Keyboard, VoiceOver, reduced-motion, contrast, zoom, focus order, and localization tests cover core investigation tasks.
- [ ] Every claim exposes cited events and limitations; gaps cannot be collapsed into an empty timeline without a warning.
- [ ] The UI capability allowlist cannot edit the database, run commands, fetch URLs, or render untrusted HTML.

## Context
Evidence honesty must survive presentation and accessibility transformations.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
