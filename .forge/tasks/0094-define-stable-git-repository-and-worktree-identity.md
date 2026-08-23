---
id: 0094
title: Define stable Git repository and worktree identity
status: backlog
agent: git-specialist
model: human
release: M4
parent: 0025
depends_on: [0007, 0018]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Identify repositories, worktrees, bare repositories, submodules, and linked worktrees without retaining remote credentials or raw paths.

## Acceptance criteria
- [ ] Identity distinguishes repository object database, worktree, selected root, and source scope.
- [ ] Remote URLs, credential helpers, config values, reflog messages, and filesystem paths are excluded or irreversibly minimized.
- [ ] Move, clone, worktree-add, submodule, bare, and repository-reinitialization fixtures define continuity behavior.

## Context
A path is not a durable Git repository identity, while remote configuration may contain secrets.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
