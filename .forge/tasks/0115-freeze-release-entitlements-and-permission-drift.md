---
id: 0115
title: Freeze release entitlements and permission drift
status: done
agent: security-auditor
model: human
release: M6
parent: 0038
depends_on: [0005, 0008, 0023]
change: pr-353
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Maintain an executable manifest of binaries, bundles, helpers, extensions, entitlements, privacy-sensitive APIs, filesystem rights, and network capabilities for every artifact.

## Acceptance criteria
- [x] CI extracts and compares signed entitlement and bundle metadata against the reviewed manifest.
- [x] New or broadened permissions fail until privacy, threat, test, and migration evidence is approved.
- [x] Release evidence proves no debug, get-task-allow, disable-library-validation, unexpected network, or overbroad sandbox exception is present.

## Context
Permission drift can invalidate a privacy review even when application code is unchanged.

## Notes
Implemented in PR #353 and squash-merged to protected `main` at
`2a18133eb7f2bdc0fe6b2482f4e56d7b9a7e8ba0`. Verified on merged `main` at `5a0bdbba25a97009d3bf520b90b83198e65da6ec`; see
[`docs/evidence/0115-permission-manifest.md`](../../docs/evidence/0115-permission-manifest.md)
for command-level device receipts and limitations.
