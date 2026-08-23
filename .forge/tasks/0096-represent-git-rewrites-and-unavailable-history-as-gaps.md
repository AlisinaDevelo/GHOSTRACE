---
id: 0096
title: Represent Git rewrites and unavailable history as gaps
status: backlog
agent: git-specialist
model: human
release: M4
parent: 0025
depends_on: [0094, 0095]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [security]
platform: any
---

## Goal
Detect force updates, detached state, garbage collection, shallow-boundary changes, replaced objects, and missing objects without inventing ancestry.

## Acceptance criteria
- [ ] The integration distinguishes observed ref movement from inferred commit ancestry.
- [ ] Missing or rewritten history emits a typed gap with the last known and current bounded state.
- [ ] Fixtures cover rebase, reset, force update, amend, gc, shallow deepen, worktree detach, and object loss.

## Context
Git history is mutable locally; a later graph cannot retroactively prove what was previously present.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
