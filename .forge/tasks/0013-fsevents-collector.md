---
id: 0013
title: Implement selected-root macOS FSEvents collector
status: done
agent: maintainer
model: human
release: M2
depends_on: [0002, 0007, 0010, 0012, 0063, 0064, 0065]
change: pr-241-8b4268676d5b59964ecb2ad65463c6f4b76b8ec4
workstream: filesystem
type: feature
priority: p0
risks: [privacy]
platform: macos
---

## Goal
Add the first live source while limiting observation to filesystem metadata under roots the user explicitly selects.

## Acceptance criteria
- [x] Root collection requires explicit opt-in.
- [x] File-level events are captured without content access.
- [x] Collector lifecycle status is visible.
- [x] Controlled create, modify, move, and delete integration tests pass.

## Context
FSEvents reports changes rather than a complete causal record. The collector must expose that limitation and retain source flags without overstating certainty.

## Notes
Implemented in PR #241 and merged to protected `main` at
`8b4268676d5b59964ecb2ad65463c6f4b76b8ec4`. The collector canonicalizes an
explicitly selected directory, binds the exact opaque root IDs to the confirmed
policy receipt, records lifecycle status, hashes callback paths without opening
or reading them, and maps controlled create/modify/move/delete callbacks to
path-free journal records. Revocation synchronously stops observation and
prevents pending events from committing. See
`docs/evidence/0013-selected-root-collector.md` for the acceptance mapping,
device receipts, and remaining scope limits.
