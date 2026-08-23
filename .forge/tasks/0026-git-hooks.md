---
id: 0026
title: Add opt-in Git hook install and uninstall
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0025]
change: null
---

## Goal
Offer convenient repository-local snapshot hooks without taking silent control of existing Git behavior.

## Acceptance criteria
- [ ] Installation requires confirmation and is idempotent.
- [ ] Existing hooks are preserved and restored.
- [ ] No global Git configuration is changed.
- [ ] Uninstall behavior is tested.

## Context
Hook management must be reversible and repository-scoped. Existing custom hooks and hook managers must not be overwritten.

## Notes
No implementation notes yet.
