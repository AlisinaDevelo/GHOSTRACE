---
id: 0025
title: Add explicit Git snapshot integration
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0007, 0018]
change: null
---

## Goal
Capture a privacy-bounded view of repository state when the user explicitly requests a Git snapshot.

## Acceptance criteria
- [ ] Snapshots contain an opaque repository ID, branch, HEAD, and status counts.
- [ ] Diffs, file content, and remote URLs are absent.
- [ ] Hostile branch and path names are parsed and rendered safely.

## Context
Repository identity must not expose remote ownership or filesystem details. Git subprocess invocation and parsing must treat all names as untrusted data.

## Notes
No implementation notes yet.
