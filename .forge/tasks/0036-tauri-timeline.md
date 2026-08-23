---
id: 0036
title: Build read-only Tauri timeline and explain UI
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0019, 0020, 0035, 0113]
change: null
workstream: service-ui
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Provide a safe desktop timeline for inspecting evidence, coverage, policy, and explanations without adding journal mutation capabilities.

## Acceptance criteria
- [ ] The interface shows coverage, gaps, policy, and collector status.
- [ ] It exposes no arbitrary shell or database access.
- [ ] Strict content-security policy and capability allowlist are enforced.
- [ ] The interface is read-only by default.

## Context
The desktop application consumes the versioned local service rather than opening the database directly. Sensitive values require deliberate, bounded rendering.

## Notes
No implementation notes yet.
