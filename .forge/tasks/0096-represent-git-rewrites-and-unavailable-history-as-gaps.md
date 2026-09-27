---
id: 0096
title: Represent Git rewrites and unavailable history as gaps
status: done
agent: git-specialist
model: human
release: M4
parent: 0025
depends_on: [0094, 0095]
change: pr-345
workstream: shell-git
type: feature
priority: p1
risks: [security]
platform: any
---

## Goal
Detect force updates, detached state, garbage collection, shallow-boundary changes, replaced objects, and missing objects without inventing ancestry.

## Acceptance criteria
- [x] The integration distinguishes observed ref movement from inferred commit ancestry.
- [x] Missing or rewritten history emits a typed gap with the last known and current bounded state.
- [x] Fixtures cover rebase, reset, force update, amend, gc, shallow deepen, worktree detach, and object loss.

## Context
Git history is mutable locally; a later graph cannot retroactively prove what was previously present.

## Notes
Implemented in PR #345 and squash-merged to protected `main` at
`190b3d142c48a7245bb649f1e64175738762f61f`. Verified on merged `main` at `4b84b424f76b441b1b328330301b22961634390c`; see
[`docs/evidence/0096-git-history-gaps.md`](../../docs/evidence/0096-git-history-gaps.md)
for command-level device receipts and limitations.
