---
id: 0024
title: Add explicit shell-wrapper metadata capture
status: done
agent: maintainer
model: human
release: M4
depends_on: [0007, 0018, 0091, 0092, 0093]
change: pr-344
workstream: shell-git
type: feature
priority: p1
risks: [privacy]
platform: any
---

## Goal
Record narrowly scoped execution metadata for commands the user deliberately runs through the GHOSTRACE wrapper.

## Acceptance criteria
- [x] Capture occurs only through the explicit run wrapper.
- [x] Executable, timing, exit status, and sanitized working directory are captured.
- [x] Arguments, environment, standard input, and output are not captured.

## Context
This integration must remain explicit and metadata-only so command secrets and terminal content never become baseline journal fields.

## Notes
Implemented in PR #344 and squash-merged to protected `main` at
`905c11e6895a63c6a5b38a2973e64686b9321b4b`. The consent-gated `ShellWrapper` records executable token, sanitized working directory, timing, and outcome for one explicit run. Verified on merged `main` at
`905c11e6895a63c6a5b38a2973e64686b9321b4b`; see
[`docs/evidence/0024-explicit-shell-wrapper.md`](../../docs/evidence/0024-explicit-shell-wrapper.md)
for command-level device receipts and limitations.
