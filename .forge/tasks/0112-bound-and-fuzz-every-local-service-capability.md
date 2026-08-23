---
id: 0112
title: Bound and fuzz every local-service capability
status: backlog
agent: test-engineer
model: human
release: M5
parent: 0035
depends_on: [0111]
change: null
workstream: service-ui
type: test
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Prove local clients cannot turn query, explanation, export, policy, or lifecycle requests into unbounded work or cross-profile data access.

## Acceptance criteria
- [ ] Each method has independent byte, item, range, runtime, concurrency, and response limits.
- [ ] Schema, state-machine, cancellation, slow-reader, connection-flood, forged-token, and cross-user cases are fuzzed or load-tested.
- [ ] Errors remain bounded and reveal neither sensitive records nor filesystem paths.

## Context
A local-only service is still an attack surface shared by every process in the user session.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
