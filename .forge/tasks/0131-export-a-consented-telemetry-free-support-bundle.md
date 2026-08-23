---
id: 0131
title: Export a consented telemetry-free support bundle
status: backlog
agent: privacy-engineer
model: human
release: M7
parent: 0125
depends_on: [0040, 0126]
change: null
workstream: operations
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Package an explicit local support artifact containing only selected redacted health evidence, manifests, versions, and failure codes.

## Acceptance criteria
- [ ] The user previews every file, field class, count, time range, and recipient warning before creation.
- [ ] The bundle excludes events, paths, URLs, commands, titles, database pages, keys, tokens, and raw logs by construction.
- [ ] Creation is atomic, permission-restricted, manifest-bound, size-limited, and covered by prohibited-data sentinel tests.

## Context
Support must not require ambient telemetry or asking users to upload their journal.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
