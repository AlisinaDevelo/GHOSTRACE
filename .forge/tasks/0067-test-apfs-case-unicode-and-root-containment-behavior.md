---
id: 0067
title: Test APFS case, Unicode, and root-containment behavior
status: done
agent: test-engineer
model: human
release: M2
parent: 0014
depends_on: [0013, 0045]
change: pr-244-37b8e177b64e98138004b7dcbcd6e2abe0afab1b
workstream: filesystem
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Define canonical comparison without inventing cross-volume path equivalence across case-sensitive, case-insensitive, and Unicode-normalizing filesystems.

## Acceptance criteria
- [x] Fixtures cover composed and decomposed Unicode, case-only renames, normalization collisions, and mixed-volume roots.
- [x] Containment uses filesystem-aware identity and rejects lexical prefix tricks.
- [x] Exported path digests remain stable only within the documented normalization and key scope.

## Context
macOS path strings do not provide a universal canonical identity across filesystem configurations.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.

## Completion

Merged implementation: PR #244 at `37b8e177b64e98138004b7dcbcd6e2abe0afab1b`.
The protected-main focused, full reproducibility, release, rustdoc, and offline
receipts are retained in `docs/evidence/0067-path-scope.md`.
