---
id: 0026
title: Add opt-in Git hook install and uninstall
status: done
agent: maintainer
model: human
release: M4
depends_on: [0025, 0097]
change: pr-362
workstream: shell-git
type: feature
priority: p2
risks: [privacy]
platform: any
---

## Goal
Offer convenient repository-local snapshot hooks without taking silent control of existing Git behavior.

## Acceptance criteria
- [x] Installation requires confirmation and is idempotent.
- [x] Existing hooks are preserved and restored.
- [x] No global Git configuration is changed.
- [x] Uninstall behavior is tested.

## Context
Hook management must be reversible and repository-scoped. Existing custom hooks and hook managers must not be overwritten.

## Notes
Implemented in PR #362 and squash-merged to protected `main` at
`5a0bdbba25a97009d3bf520b90b83198e65da6ec`. Verified on merged `main` at `5a0bdbba25a97009d3bf520b90b83198e65da6ec`; see
[`docs/evidence/0026-git-hook-chaining.md`](../../docs/evidence/0026-git-hook-chaining.md)
for command-level device receipts and limitations.
