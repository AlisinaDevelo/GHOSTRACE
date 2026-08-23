---
id: 0042
title: Evaluate optional Endpoint Security actor attribution
status: backlog
agent: maintainer
model: human
release: M6
depends_on: [0017, 0019, 0037, 0124]
change: null
workstream: release-scale
type: spike
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Run a bounded go-or-no-go evaluation of an optional, entitlement-gated macOS mode that can attach direct process evidence to filesystem changes.

## Acceptance criteria
- [ ] The Apple entitlement, signing, system-extension, Full Disk Access, installation, and review requirements are documented; unavailable entitlement or authorization paths have a checked-in evidence record instead of being silently skipped.
- [ ] A notify-only prototype measures actor-attribution accuracy, event volume, overhead, PID-reuse handling, and coverage gaps on selected roots.
- [ ] The privacy review compares the higher-privilege mode with the low-permission FSEvents baseline.
- [ ] A recorded decision either scopes the feature behind separate explicit consent or rejects it without weakening the baseline product.

## Context
FSEvents cannot reliably identify the responsible process. Endpoint Security can provide stronger evidence, but its entitlement and permission footprint must remain optional and must not become a hidden prerequisite for the local journal.

## Notes
No implementation notes yet.
