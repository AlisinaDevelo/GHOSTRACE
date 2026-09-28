---
id: 0170
title: Publish a guided live demo
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0161, 0167, 0168, 0169]
change: null
workstream: foundation
type: docs
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Show GHOSTRACE working end to end on a real Mac in a few minutes: record a wrapped command, a folder change, and a Git snapshot, then read the timeline and an explanation.

## Acceptance criteria
- [ ] A script runs the demo in a temporary workspace and cleans it up, with no network and no personal data.
- [ ] The documented walkthrough shows real output from the reference device.
- [ ] The demo states what was not observed and why (coverage limits and gaps).

## Context
Seeing the product run is the fastest way to judge whether the evidence model is useful.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
