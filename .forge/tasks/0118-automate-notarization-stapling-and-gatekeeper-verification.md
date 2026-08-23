---
id: 0118
title: Automate notarization, stapling, and Gatekeeper verification
status: backlog
agent: release-engineer
model: human
release: M6
parent: 0038
depends_on: [0116, 0117]
change: null
workstream: release-scale
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Sign with the correct Developer ID identities, enable hardened runtime, notarize with current Apple tooling, staple tickets, and verify the exact distributed artifacts.

## Acceptance criteria
- [ ] The pipeline uses notarytool or the supported Notary API and stores credentials outside build logs and artifacts.
- [ ] codesign, stapler, spctl, quarantine, clean-machine install, first launch, upgrade, and uninstall checks cover every package type.
- [ ] Notary warnings, rejected submissions, expired or revoked credentials, changed entitlements, and ticket lookup failure stop release.

## Context
Apple requires hardened runtime for notarization and no longer accepts altool uploads.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
