---
id: 0044
title: Enforce an offline network-denial CI lane
status: done
agent: security-auditor
model: human
release: M0
parent: 0005
depends_on: [0003, 0004]
change: null
workstream: privacy
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Prove that the fixture-only product path can build and execute its runtime tests without opening an outbound network surface.

## Acceptance criteria
- [x] A pinned CI mechanism denies network access while running fixture, explanation, and export tests.
- [x] A canary test attempts a connection and proves the denial mechanism is active rather than silently skipped.
- [x] If hosted-runner constraints prevent enforcement, a checked-in decision record names the failed mechanism and an equivalent reproducible test.

## Context
Dependency download and runtime execution are separate phases; the denial applies to the exercised product path.

## Notes
Implemented in PR #170. The pre-merge implementation commit was
`477f56f6074596ff49608a69eddfeeea788289cb`; it merged to protected `main` at
`85f653aca45b48776853eef83d3a3e183063d54e` on 2026-08-24. Evidence artifacts are
`GHOSTRACE-0044-OFFLINE-CODE-477F56F`, `GHOSTRACE-0044-MERGED-85F653A`,
`GHOSTRACE-0044-LOCAL-PREMERGE-3B13EBF`, `GHOSTRACE-0044-LOCAL-MERGED-D0BD584`,
`GHOSTRACE-0044-LOCAL-DENY-261704F`, `GHOSTRACE-0044-LOCAL-MARKER-7C74BC7`,
and `GHOSTRACE-0044-DOCKER-DAEMON-591C8BE`. The retained report is
`docs/evidence/0044-offline-network-denial.md`.

The same enforced reproduction passed before and after merge on a MacBookPro17,1
(Apple M1, 8 GB), macOS 26.6.2 (25G83), arm64, Darwin 25.6.0, with rustc/cargo
1.88.0. The local Docker CLI had no running daemon; ADR 0004 records that
constraint and the passing sandbox-exec equivalent. The fixture-only scope does
not claim live capture or production hardware coverage. Completion requires the
acceptance evidence above; issue closure must link the report, artifacts, logs,
limitations, and merged SHA.
