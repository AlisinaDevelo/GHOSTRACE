---
id: 0031
title: Implement Chromium top-level navigation collector
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0007, 0029, 0030, 0105, 0106]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Record explicitly enabled, top-level browser navigation metadata while removing common secret-bearing URL components before persistence.

## Acceptance criteria
- [ ] Collection requires explicit installation and enablement.
- [ ] Only top-level committed navigation is recorded.
- [ ] URL userinfo, query strings, and fragments are removed by default.
- [ ] No content scripts are used and incognito collection is disabled.

## Context
The collector must never fetch pages or retain page content. Sanitization occurs before an event crosses the policy and persistence boundary.

## Notes
No implementation notes yet.
