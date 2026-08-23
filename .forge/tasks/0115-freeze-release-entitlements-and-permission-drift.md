---
id: 0115
title: Freeze release entitlements and permission drift
status: backlog
agent: security-auditor
model: human
release: M6
parent: 0038
depends_on: [0005, 0008, 0023]
change: null
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Maintain an executable manifest of binaries, bundles, helpers, extensions, entitlements, privacy-sensitive APIs, filesystem rights, and network capabilities for every artifact.

## Acceptance criteria
- [ ] CI extracts and compares signed entitlement and bundle metadata against the reviewed manifest.
- [ ] New or broadened permissions fail until privacy, threat, test, and migration evidence is approved.
- [ ] Release evidence proves no debug, get-task-allow, disable-library-validation, unexpected network, or overbroad sandbox exception is present.

## Context
Permission drift can invalidate a privacy review even when application code is unchanged.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
