---
id: 0171
title: Record frontmost-app changes from the CLI with explicit consent
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0027, 0165]
change: null
workstream: frontmost
type: feature
priority: p1
risks: [privacy]
platform: macos
---

## Goal
Let a person record which application was in front, for a bounded time they choose, into their live journal.

## Acceptance criteria
- [ ] `ghostrace live apps` shows what is recorded (bundle ID, developer-set name and version, signing class, dwell) and refuses without confirmation.
- [ ] Activations, inferred closures, and a clean stop are journaled; Ctrl-C and sleep or lock boundaries are recorded as coverage, not as time in an app.
- [ ] Excluded bundle IDs keep no identity, and no window title, document, URL, or path appears in the journal or CLI output.

## Context
The collector in 0027 is a library adapter; this is the first time frontmost context reaches a person's timeline.

## Notes
The adapter must pump the calling thread's run loop: `NSWorkspace.frontmostApplication` goes stale in a command-line process that does not (verified on the reference M1).
Completion requires the acceptance evidence above; issue closure alone is not evidence.
