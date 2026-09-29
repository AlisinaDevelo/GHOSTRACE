---
id: 0168
title: Record explicit Git snapshots from the CLI
status: done
agent: git-specialist
model: human
release: M4
depends_on: [0165, 0025, 0096]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Let a person record a metadata-only snapshot of a repository they name and see history transitions between snapshots.

## Acceptance criteria
- [x] `ghostrace git-snapshot [path]` records the hardened adapter's snapshot under an explicit policy and root.
- [x] Consecutive snapshots of the same repository record their history transition, including gaps for rewritten or missing history.
- [x] No ref names, paths, remotes, or file names appear in the journal or CLI output.

## Context
The adapter and transition contract exist; this makes them usable and journals their output.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
