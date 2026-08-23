---
id: 0093
title: Red-team shell secret leakage
status: backlog
agent: security-auditor
model: human
release: M4
parent: 0024
depends_on: [0091]
change: null
workstream: shell-git
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Prove that common credential, environment, prompt, path, process-title, diagnostic, and crash-report channels do not enter retained shell evidence.

## Acceptance criteria
- [ ] The corpus covers tokens in arguments, environment, stdin, stdout, stderr, executable names, working paths, and failure messages.
- [ ] Journal, logs, errors, exports, panic output, and process inspection are checked for unique sentinels.
- [ ] Any unavoidable operating-system exposure is documented separately from GHOSTRACE retention claims.

## Context
The wrapper boundary is valuable only if its negative data contract survives failure paths.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
