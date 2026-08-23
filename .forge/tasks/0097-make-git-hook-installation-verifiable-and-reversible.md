---
id: 0097
title: Make Git hook installation verifiable and reversible
status: backlog
agent: git-specialist
model: human
release: M4
parent: 0026
depends_on: [0094, 0095]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Install optional hooks without overwriting user hooks, following untrusted indirection, or leaving an ambiguous partial configuration.

## Acceptance criteria
- [ ] Plan, install, verify, upgrade, disable, and uninstall operations are idempotent and show exact affected files.
- [ ] Existing hooks, core.hooksPath, worktrees, symlinks, ownership, modes, and concurrent edits are preserved or cause refusal.
- [ ] A signed or checksummed shim delegates safely and uninstall removes only artifacts whose identity still matches.

## Context
Hook management is a filesystem mutation and must preserve user configuration exactly.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
