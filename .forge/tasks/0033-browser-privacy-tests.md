---
id: 0033
title: Add browser privacy matrix and private-mode regression suite
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0031, 0032, 0108, 0109]
change: null
workstream: browser
type: test
priority: p1
risks: [privacy]
platform: any
---

## Goal
Prove browser collectors honor field-level privacy rules across normal, secret-bearing, malformed, and private contexts.

## Acceptance criteria
- [ ] Private navigation stores no URL event.
- [ ] Policy counters remain visible without retaining private URLs.
- [ ] A token-bearing URL corpus is sanitized.
- [ ] Any future private-mode opt-in requires a separate ADR.

## Context
The matrix should cover transport, browser mode, event type, permissions, retained fields, policy outcome, and observable diagnostics.

## Notes
No implementation notes yet.
