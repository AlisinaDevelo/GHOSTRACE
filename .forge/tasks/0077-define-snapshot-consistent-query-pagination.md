---
id: 0077
title: Define snapshot-consistent query pagination
status: done
agent: database-expert
model: human
release: M3
parent: 0018
depends_on: [0010, 0012]
change: pr-274
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Return stable bounded pages from a growing journal without skipping, duplicating, or exposing events outside the authorized query scope.

## Acceptance criteria
- [x] Page tokens bind query parameters, policy scope, schema version, snapshot boundary, and stable ordering keys.
- [x] Expired, forged, cross-profile, or future tokens fail with bounded errors.
- [x] Concurrent ingest, deletion, retention, and migration tests prove the documented snapshot semantics.

## Context
Offset pagination over a changing evidence journal is neither stable nor tamper-resistant.

## Notes
Implementation PR #274 (`48c7123c6010408bc9c184843f4b9bdb2cd9952a`) is merged
to protected `main` at `bd49f1d0d23cded0b10220ee6922f34572b30bf3`. The query
API uses an encrypted page token that binds the policy scope, complete filter
shape, schema versions, expiry, snapshot boundary, and stable ordering key.
Debug and release matrices were rerun from that exact protected SHA on the
named device. Retained acceptance evidence is
`docs/evidence/0077-snapshot-consistent-query-pagination.md`; the public issue
remains open until this evidence change is merged and linked.
