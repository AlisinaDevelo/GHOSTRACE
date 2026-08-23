---
id: 0143
title: Benchmark privacy leakage across every artifact
status: backlog
agent: privacy-engineer
model: human
release: M9
parent: 0139
depends_on: [0132, 0140]
change: null
workstream: research
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Measure prohibited and sensitive information exposure in journals, sidecars, memory, diagnostics, support bundles, exports, archives, backups, crash reports, and release artifacts.

## Acceptance criteria
- [ ] A labeled sentinel corpus covers direct values, derivatives, lengths, counts, timing, identifiers, and cross-record inference.
- [ ] Scanning and manual review report artifact, field, transformation, retention, access boundary, and false-positive limitations.
- [ ] Any baseline-prohibited value is a release-blocking regression until fixed and covered by a minimized test.

## Context
Privacy evaluation must include metadata and operational residue, not just declared event fields.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
