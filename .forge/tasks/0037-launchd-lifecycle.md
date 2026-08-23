---
id: 0037
title: Add launchd user-agent lifecycle and permission UX
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0013, 0027, 0030, 0035]
change: null
---

## Goal
Make background collection explicit, controllable, recoverable, and understandable as an unprivileged macOS user service.

## Acceptance criteria
- [ ] Install, start, stop, and status operations are explicit.
- [ ] The service does not require root and uses crash backoff.
- [ ] No collection occurs before consent.
- [ ] TCC failures are explained clearly.
- [ ] Uninstall preserves journal and export data.

## Context
Lifecycle state must distinguish not installed, stopped, denied, degraded, and running. Removing the service must not silently remove user data.

## Notes
No implementation notes yet.
