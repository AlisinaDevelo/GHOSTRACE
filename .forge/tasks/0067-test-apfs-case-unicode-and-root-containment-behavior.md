---
id: 0067
title: Test APFS case, Unicode, and root-containment behavior
status: backlog
agent: test-engineer
model: human
release: M2
parent: 0014
depends_on: [0013, 0045]
change: null
workstream: filesystem
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Define canonical comparison without inventing cross-volume path equivalence across case-sensitive, case-insensitive, and Unicode-normalizing filesystems.

## Acceptance criteria
- [ ] Fixtures cover composed and decomposed Unicode, case-only renames, normalization collisions, and mixed-volume roots.
- [ ] Containment uses filesystem-aware identity and rejects lexical prefix tricks.
- [ ] Exported path digests remain stable only within the documented normalization and key scope.

## Context
macOS path strings do not provide a universal canonical identity across filesystem configurations.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
