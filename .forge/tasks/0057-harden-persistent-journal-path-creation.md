---
id: 0057
title: Harden persistent journal path creation
status: ready
agent: security-auditor
model: human
release: M1
parent: 0009
depends_on: [0003, 0004]
change: null
workstream: storage
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Create the production journal and containing directory without following attacker-controlled links or inheriting unsafe ownership and modes.

## Acceptance criteria
- [ ] Creation rejects symlinks, non-regular files, unexpected owners, hard-link anomalies, and group or world access.
- [ ] The database directory, database, WAL, SHM, temporary, backup, and export files receive verified restrictive modes.
- [ ] Race-oriented tests replace path components between validation and open and prove fail-closed behavior.

## Context
The fixture-only open path is not a safe production file-creation boundary until TOCTOU and sidecar handling are closed.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
