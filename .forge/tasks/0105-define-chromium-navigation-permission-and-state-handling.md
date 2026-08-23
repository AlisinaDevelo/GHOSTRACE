---
id: 0105
title: Define Chromium navigation permission and state handling
status: backlog
agent: browser-engineer
model: human
release: M5
parent: 0031
depends_on: [0007, 0029, 0103, 0104]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Collect only explicitly approved top-level navigation state with the minimum extension permissions and honest tab and frame lifecycle semantics.

## Acceptance criteria
- [ ] The extension requests and documents the minimum permissions for the enabled event classes.
- [ ] Top-level commit, replacement, redirect, history update, tab close, browser restart, and missing-host behavior have deterministic outcomes.
- [ ] Subframes, form data, page content, DOM, cookies, credentials, downloads, and network bodies are structurally excluded.

## Context
A navigation event is bounded context, not a license to observe page activity.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
