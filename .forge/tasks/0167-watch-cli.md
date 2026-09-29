---
id: 0167
title: Watch a selected folder from the CLI with explicit consent
status: done
agent: macos-engineer
model: human
release: M4
depends_on: [0165, 0013, 0016]
change: null
workstream: filesystem
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Run the selected-root FSEvents collector from the CLI for a folder the user names, after showing and confirming exactly what will be recorded.

## Acceptance criteria
- [x] `ghostrace watch <folder>` shows the consent preview (root, retained fields, limits) and refuses without confirmation.
- [x] Events, gaps, and a clean stop are journaled; Ctrl-C stops collection and records a collector-stopped event.
- [x] The journal directory and internal artifacts are never observed as user changes.

## Context
Selected-root collection is implemented and verified as a library; this exposes it without broadening it.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
