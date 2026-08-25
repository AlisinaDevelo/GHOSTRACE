---
id: 0070
title: Persist one replay boundary per source and volume
status: done
agent: database-expert
model: human
release: M2
parent: 0015
depends_on: [0061, 0063, 0066]
change: pr-252
workstream: filesystem
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Store the last durably covered FSEvents position together with source, volume, policy, and stream configuration evidence.

## Acceptance criteria
- [x] Cursor advancement commits atomically with every acknowledged event or gap batch.
- [x] Changed root, latency, since-when, file-event, or exclusion settings invalidate or explicitly fork the boundary.
- [x] Restart tests prove no acknowledged interval is skipped and duplicates remain idempotent.

## Context
A numeric event ID without its volume and stream contract is not a safe recovery cursor.

## Notes
Implemented as the versioned `ReplayBoundary` and `0004_replay_boundary`
migration. Completion requires the acceptance evidence above; issue closure alone
is not evidence. Full source-loss restart recovery remains task 0015 and its
children.
