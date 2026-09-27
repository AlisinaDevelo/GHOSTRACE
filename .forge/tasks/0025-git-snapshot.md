---
id: 0025
title: Add explicit Git snapshot integration
status: done
agent: maintainer
model: human
release: M4
depends_on: [0007, 0018, 0094, 0095, 0096]
change: pr-354
workstream: shell-git
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Capture a privacy-bounded view of repository state when the user explicitly requests a Git snapshot.

## Acceptance criteria
- [x] Snapshots contain an opaque repository ID, branch, HEAD, and status counts.
- [x] Diffs, file content, and remote URLs are absent.
- [x] Hostile branch and path names are parsed and rendered safely.

## Context
Repository identity must not expose remote ownership or filesystem details. Git subprocess invocation and parsing must treat all names as untrusted data.

## Notes
Implemented in PR #354 and squash-merged to protected `main` at
`24b2af0a8f88bca68d0153b4df29d4a0ca749bb6`. Verified on merged `main` at `4b84b424f76b441b1b328330301b22961634390c`; see
[`docs/evidence/0025-git-snapshot-adapter.md`](../../docs/evidence/0025-git-snapshot-adapter.md)
for command-level device receipts and limitations.
