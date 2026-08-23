---
id: 0095
title: Minimize Git refs, object IDs, and snapshot fields
status: backlog
agent: privacy-engineer
model: human
release: M4
parent: 0025
depends_on: [0094]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define which commit, tree, index, worktree-dirty, branch-class, and operation facts are useful without retaining sensitive names or content.

## Acceptance criteria
- [ ] Object IDs use validated algorithm-aware formats and never cause object content reads by default.
- [ ] Ref names, commit messages, authors, remotes, diffs, patches, filenames, and untracked content are excluded from the baseline.
- [ ] Snapshots expose source limitations for partial clones, replace refs, shallow history, submodules, and alternate object databases.

## Context
Git metadata can reveal identity and project names even when file content is not read.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
