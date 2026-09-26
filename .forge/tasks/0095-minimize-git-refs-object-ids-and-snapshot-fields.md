---
id: 0095
title: Minimize Git refs, object IDs, and snapshot fields
status: done
agent: privacy-engineer
model: human
release: M4
parent: 0025
depends_on: [0094]
change: pr-326
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define which commit, tree, index, worktree-dirty, branch-class, and operation facts are useful without retaining sensitive names or content.

## Acceptance criteria
- [x] Object IDs use validated algorithm-aware formats and never cause object content reads by default.
- [x] Ref names, commit messages, authors, remotes, diffs, patches, filenames, and untracked content are excluded from the baseline.
- [x] Snapshots expose source limitations for partial clones, replace refs, shallow history, submodules, and alternate object databases.

## Context
Git metadata can reveal identity and project names even when file content is not read.

## Notes
Implemented in PR #326 and squash-merged to protected `main` at
`743fc17ea9ef9c410e6869cd2cd9ccb73223a30d`. The metadata-only snapshot contract retains no ref names, messages, authors, remotes, paths, or content and requires explicit source limitations. Verified on merged `main` at
`905c11e6895a63c6a5b38a2973e64686b9321b4b`; see
[`docs/evidence/0095-git-snapshot-minimization.md`](../../docs/evidence/0095-git-snapshot-minimization.md)
for command-level device receipts and limitations.
