---
id: 0091
title: Freeze the explicit shell metadata schema
status: backlog
agent: privacy-engineer
model: human
release: M4
parent: 0024
depends_on: [0007, 0018]
change: null
workstream: shell-git
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define the minimum execution metadata retained only for commands deliberately routed through the wrapper.

## Acceptance criteria
- [ ] The schema permits executable identity, sanitized working-directory identity, start and end time, exit status, signal, and wrapper session only.
- [ ] Arguments, environment, standard input, output, shell history, aliases, and expanded command text are structurally impossible to retain.
- [ ] Every field has semantic validation, sensitivity classification, and adversarial fixtures.

## Context
Shell integrations encounter credentials routinely, so minimization must be enforced by type rather than convention.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
