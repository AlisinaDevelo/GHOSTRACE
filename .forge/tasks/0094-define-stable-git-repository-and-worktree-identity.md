---
id: 0094
title: Define stable Git repository and worktree identity
status: done
agent: git-specialist
model: human
release: M4
parent: 0025
depends_on: [0007, 0018]
change: pr-324
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Identify repositories, worktrees, bare repositories, submodules, and linked worktrees without retaining remote credentials or raw paths.

## Acceptance criteria
- [x] Identity distinguishes repository object database, worktree, selected root, and source scope.
- [x] Remote URLs, credential helpers, config values, reflog messages, and filesystem paths are excluded or irreversibly minimized.
- [x] Move, clone, worktree-add, submodule, bare, and repository-reinitialization fixtures define continuity behavior.

## Context
A path is not a durable Git repository identity, while remote configuration may contain secrets.

## Notes
Implemented in PR #324 and squash-merged to protected `main` at
`2882c1923ace639b72a9d0582dc7d8545a190246`. The path-free contract and
deterministic transition matrix are verified on the merged revision; see
[`docs/evidence/0094-git-repository-worktree-identity.md`](../../docs/evidence/0094-git-repository-worktree-identity.md)
for command-level device receipts and limitations.
