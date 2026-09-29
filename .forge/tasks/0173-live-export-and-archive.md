---
id: 0173
title: Export and archive a live journal
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0165, 0020, 0022]
change: null
workstream: explain-export
type: feature
priority: p2
risks: [privacy]
platform: macos
---

## Goal
Let a person take their live journal out as a validated JSONL export and, optionally, a Parquet archive.

## Acceptance criteria
- [ ] `ghostrace live export` uses the same preview, plan and snapshot confirmation, and 0600 atomic publish as the fixture export, unlocked with the home's key custody.
- [ ] The export passes `ghostrace validate`, and with the `parquet` feature an archive written from it passes `verify-archive`.
- [ ] The export destination is refused inside the GHOSTRACE home and is never observed by a running watch.

## Context
Export and archive exist for fixture journals only; the live home has no way out except the timeline.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
