---
id: 0148
title: Decide on an isolated plug-in execution model
status: backlog
agent: security-researcher
model: human
release: M10
parent: 0146
depends_on: [0137, 0147]
change: null
workstream: ecosystem
type: spike
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Compare no third-party execution, subprocess isolation, sandbox profiles, XPC, and WASI-style runtimes against macOS permission, signing, capability, performance, and recovery requirements.

## Acceptance criteria
- [ ] Threat prototypes measure filesystem, process, network, journal, memory, denial-of-service, escape, update, and crash containment.
- [ ] The selected model passes a capability-denial corpus or the ADR records a no-go and keeps adapters first-party only.
- [ ] No design relies on an undocumented sandbox guarantee or grants a plug-in the user's full GHOSTRACE process authority.

## Context
Isolation must be demonstrated on the target platform rather than inferred from a process boundary.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
