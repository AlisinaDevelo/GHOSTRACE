---
id: 0097
title: Make Git hook installation verifiable and reversible
status: done
agent: git-specialist
model: human
release: M4
parent: 0026
depends_on: [0094, 0095]
change: pr-355
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Install optional hooks without overwriting user hooks, following untrusted indirection, or leaving an ambiguous partial configuration.

## Acceptance criteria
- [x] Plan, install, verify, upgrade, disable, and uninstall operations are idempotent and show exact affected files.
- [x] Existing hooks, core.hooksPath, worktrees, symlinks, ownership, modes, and concurrent edits are preserved or cause refusal.
- [x] A signed or checksummed shim delegates safely and uninstall removes only artifacts whose identity still matches.

## Context
Hook management is a filesystem mutation and must preserve user configuration exactly.

## Notes
Implemented in PR #355 and squash-merged to protected `main` at
`4b84b424f76b441b1b328330301b22961634390c`. Verified on merged `main` at `4b84b424f76b441b1b328330301b22961634390c`; see
[`docs/evidence/0097-git-hook-lifecycle.md`](../../docs/evidence/0097-git-hook-lifecycle.md)
for command-level device receipts and limitations.
