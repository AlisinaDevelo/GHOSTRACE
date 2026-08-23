---
id: 0123
title: Practice disaster recovery and incident response
status: backlog
agent: incident-responder
model: human
release: M6
parent: 0040
depends_on: [0023, 0037, 0089]
change: null
workstream: release-scale
type: test
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Exercise corrupted journal, lost key, compromised release key, malicious update, broken migration, privacy leak, source runaway, and unavailable Apple service scenarios.

## Acceptance criteria
- [ ] Each scenario has detection, containment, preservation, user communication, repair, rollback, and post-incident criteria.
- [ ] The drill runs against disposable signed artifacts and synthetic journals and records measured recovery and data-loss bounds.
- [ ] Runbooks name actions that require user consent and refuse destructive automatic recovery.

## Context
Incident readiness is practiced behavior, not a document review.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
