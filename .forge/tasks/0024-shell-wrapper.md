---
id: 0024
title: Add explicit shell-wrapper metadata capture
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0007, 0018]
change: null
---

## Goal
Record narrowly scoped execution metadata for commands the user deliberately runs through the GHOSTRACE wrapper.

## Acceptance criteria
- [ ] Capture occurs only through the explicit run wrapper.
- [ ] Executable, timing, exit status, and sanitized working directory are captured.
- [ ] Arguments, environment, standard input, and output are not captured.

## Context
This integration must remain explicit and metadata-only so command secrets and terminal content never become baseline journal fields.

## Notes
No implementation notes yet.
