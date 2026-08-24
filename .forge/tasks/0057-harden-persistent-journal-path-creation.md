---
id: 0057
title: Harden persistent journal path creation
status: done
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
- [x] Creation rejects symlinks, non-regular files, unexpected owners, hard-link anomalies, and group or world access.
- [x] The database directory, database, WAL, SHM, temporary, backup, and export files receive verified restrictive modes.
- [x] Race-oriented tests replace path components between validation and open and prove fail-closed behavior.

## Context
The fixture-only open path is not a safe production file-creation boundary until TOCTOU and sidecar handling are closed.

## Notes
Implemented in the 2026-08-24 device pipe. The path boundary is covered by unit
tests for symlink, non-regular, hard-link, unsafe-mode, parent-replacement, and
artifact-mode failures; the file-backed vertical slice rechecks SQLite sidecars
after migration and each committed ingest. Completion still requires the merged-SHA
evidence receipt in `docs/evidence/0057-journal-path-hardening.md`; issue closure
alone is not evidence.
