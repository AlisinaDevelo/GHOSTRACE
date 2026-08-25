---
id: 0005
title: Add privacy regression and network-surface checks
status: done
agent: maintainer
model: human
release: M0
depends_on: [0003, 0004, 0043, 0044]
change: null
workstream: privacy
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Make the privacy contract executable by testing prohibited fields and the absence of unexpected network behavior.

## Acceptance criteria
- [x] Regression fixtures prove prohibited fields are absent.
- [x] Dependency and network policy is documented.
- [x] Linux CI runs fixture tests inside an enforced network-denied environment; if runner restrictions block that mechanism, a checked-in decision record captures the failed mechanism and the equivalent enforced offline test.

## Context
Checks must cover sensitive command, browser, filesystem, and application fields before live collection is enabled.

## Notes
The parent gate is complete through child evidence `GHOSTRACE-0043-PRIVACY-CORPUS-V1` and
`GHOSTRACE-0044-OFFLINE-CODE-477F56F`. On protected `main` commit
`4a8f2493ec9955f2724a866fc562008cbd68b790`, the target device reran the privacy
corpus in debug and release, the macOS sandbox-enforced offline lane, the full
locked debug suite, the full locked release suite, Clippy, roadmap/index parity,
and the dependency/network surface checks. Receipts are retained in
`docs/evidence/0005-privacy-network-checks.md` with SHA-256 digests and explicit
Docker/Linux availability limits. The product remains fixture-only and does not
enable live capture or a network client.
