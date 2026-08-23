---
id: 0027
title: Implement NSWorkspace frontmost-app collector
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0006, 0007, 0010, 0012, 0098]
change: null
workstream: frontmost
type: feature
priority: p1
risks: [privacy]
platform: macos
---

## Goal
Record opt-in application focus transitions using the least invasive macOS interface and a deliberately narrow payload.

## Acceptance criteria
- [ ] Collection is opt-in and stores bundle ID, application name, and version only.
- [ ] The collector uses no Accessibility permission.
- [ ] Window titles, document names, and root access are absent.

## Context
Application attribution helps establish user context around changes without observing screen content or document-level activity.

## Notes
No implementation notes yet.
