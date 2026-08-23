---
id: 0107
title: Model bookmark snapshots as bounded diffs
status: backlog
agent: browser-engineer
model: human
release: M5
parent: 0032
depends_on: [0103, 0105, 0106]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Represent explicitly requested bookmark-folder snapshots and changes without retaining titles, notes, full URLs, or unrelated profile content by default.

## Acceptance criteria
- [ ] The user selects folders and visible retained fields before the first snapshot.
- [ ] Snapshot identity, insert, remove, move, and bounded origin change semantics are versioned and deterministic.
- [ ] Large, cyclic-looking, duplicate, malformed, and concurrently edited trees produce bounded results or explicit gaps.

## Context
Bookmark data is durable browsing history and needs a separate consent and minimization profile.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
