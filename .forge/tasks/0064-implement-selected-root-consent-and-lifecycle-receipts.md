---
id: 0064
title: Implement selected-root consent and lifecycle receipts
status: done
agent: privacy-engineer
model: human
release: M2
parent: 0013
depends_on: [0007, 0012]
change: pr-238-af362e41c402a155fbad56a75882920b3d02752b
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Require an explicit, inspectable grant for every watched root and record enable, pause, scope-change, and disable transitions.

## Acceptance criteria
- [x] The user sees canonical root identity, exclusions, retained fields, and known coverage limits before enabling capture.
- [x] A receipt binds the root scope to an immutable policy version without storing path content in diagnostics.
- [x] Revocation stops observation and produces a bounded terminal status before the command returns.

## Context
Filesystem permission and product consent are separate gates; both must be visible.

## Notes
`ConsentPreview::from_policy` now renders a bounded scope summary and requires an
explicit, consumed confirmation before `grant_preview` can create an active receipt.
Receipts retain only the immutable policy identity/version and scope digest; revocation
is synchronous and terminal for the current observation session. Device receipts and
scope limits are recorded in `docs/evidence/0064-selected-root-consent.md`.
