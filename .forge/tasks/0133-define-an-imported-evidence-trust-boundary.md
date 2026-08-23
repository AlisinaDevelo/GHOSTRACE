---
id: 0133
title: Define an imported-evidence trust boundary
status: backlog
agent: security-auditor
model: human
release: M8
parent: 0132
depends_on: [0125]
change: null
workstream: interoperability
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Accept selected offline evidence bundles only through a strict origin, schema, integrity, policy, size, and provenance validation boundary.

## Acceptance criteria
- [ ] Import never assigns native collector origin, direct evidence level, local policy history, or continuous coverage to external records.
- [ ] Preview reports format, signer or unsigned state, sources, ranges, gaps, fields, counts, compatibility, and rejected content before persistence.
- [ ] Malformed, oversized, conflicting, cyclic, duplicate, decompression-bomb, and partial bundles fail safely in bounded resources.

## Context
Offline import is useful for research and migration but external claims remain untrusted evidence.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
