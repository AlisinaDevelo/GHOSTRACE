---
id: 0165
title: Operate a durable live journal from the CLI
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0164, 0018, 0019, 0020]
change: null
workstream: foundation
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Give a person a real journal on their Mac: create it, inspect its status, list a timeline, and explain events, without writing Rust.

## Acceptance criteria
- [ ] `ghostrace init` creates a journal in a private directory with explicit key custody and prints what is and is not recorded.
- [ ] `ghostrace status` and `ghostrace timeline` report sources, counts, coverage, and gaps without printing paths or payload secrets.
- [ ] `ghostrace explain` works on live journals with the same claim grammar and gap warnings as the fixture path.

## Context
The library adapters exist but only tests can drive them; a usable CLI is the first time GHOSTRACE runs for a person.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
