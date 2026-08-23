---
id: 0048
title: Pin the reproducible developer and fixture toolchain
status: ready
agent: devops-engineer
model: human
release: M0
depends_on: [0003, 0006]
change: null
workstream: foundation
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Make local and CI planning, schema validation, fixture generation, and Rust verification reproducible from documented pinned inputs.

## Acceptance criteria
- [ ] Toolchain versions and install sources are pinned or checksum-verified.
- [ ] Synthetic fixtures carry a generator version and deterministic seed without containing user data.
- [ ] A clean-machine smoke procedure reproduces the schema, demo, export, roadmap, and test evidence.

## Context
Reproducibility applies to planning and fixtures as well as compiled artifacts.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
