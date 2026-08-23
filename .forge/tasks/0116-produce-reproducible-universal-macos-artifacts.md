---
id: 0116
title: Produce reproducible universal macOS artifacts
status: backlog
agent: devops-engineer
model: human
release: M6
parent: 0038
depends_on: [0115]
change: null
workstream: release-scale
type: feature
priority: p0
risks: [security]
platform: macos
---

## Goal
Build Intel and Apple silicon CLI, service, native host, and application artifacts from locked inputs with reproducibility evidence and explained residual variance.

## Acceptance criteria
- [ ] Build parameters, Rust and Apple toolchains, SDK, dependencies, generated files, timestamps, and architecture merge steps are pinned or recorded.
- [ ] Independent rebuilds compare normalized artifact digests and report every nondeterministic field.
- [ ] The universal artifacts pass architecture, minimum-OS, signature, entitlement, runtime, and fixture smoke tests on supported systems.

## Context
Reproducibility is a measured property and cannot be claimed solely from a lockfile.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
