---
id: 0035
title: Expose versioned Unix-domain local service API
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0010, 0018, 0020, 0029, 0111, 0112]
change: null
workstream: service-ui
type: feature
priority: p1
risks: [security]
platform: macos
---

## Goal
Expose a narrow local protocol so the command line and future desktop interface share one writer and consistent query semantics.

## Acceptance criteria
- [ ] The service maintains one writer.
- [ ] The Unix socket uses mode 0600.
- [ ] Requests and responses are versioned and bounded.
- [ ] No TCP listener is created.
- [ ] Command-line and desktop queries share behavior.

## Context
The service API is local-only and read-oriented by default. It must not expose arbitrary shell commands, raw database access, or unrestricted filesystem operations.

## Notes
No implementation notes yet.
