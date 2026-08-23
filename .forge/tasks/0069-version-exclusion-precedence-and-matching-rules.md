---
id: 0069
title: Version exclusion precedence and matching rules
status: backlog
agent: privacy-engineer
model: human
release: M2
parent: 0014
depends_on: [0007, 0050]
change: null
workstream: filesystem
type: feature
priority: p0
risks: [privacy, security]
platform: macos
---

## Goal
Specify how root, subtree, file-kind, application, temporary-file, VCS, and user-defined exclusions combine before persistence.

## Acceptance criteria
- [ ] A deterministic precedence table covers allow, deny, redact, and summarize outcomes.
- [ ] Policy updates re-evaluate future events only and preserve the version used for existing evidence.
- [ ] Property tests cover overlapping, nested, escaped, case-variant, and empty patterns without catastrophic matching time.

## Context
Exclusions are a privacy control and must not depend on rule ordering accidents.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
