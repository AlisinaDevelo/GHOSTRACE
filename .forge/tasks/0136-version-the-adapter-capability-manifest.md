---
id: 0136
title: Version the adapter capability manifest
status: backlog
agent: api-designer
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
Require every first- or third-party adapter to declare source identity, event classes, retained and forbidden fields, permissions, network use, bounds, cursor semantics, gaps, and compatibility.

## Acceptance criteria
- [ ] A strict signed or checksummed manifest schema covers capabilities, constraints, versions, platform, and evidence quality.
- [ ] Policy admission compares requested capabilities with user-approved scope before the adapter can emit events.
- [ ] Unknown, broadened, downgraded, or mismatched manifests disable the adapter and require review or reconfirmation.

## Context
Adapter code should not be trusted to accurately describe its own privilege after admission.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
