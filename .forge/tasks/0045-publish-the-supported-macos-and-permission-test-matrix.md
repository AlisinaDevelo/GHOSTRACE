---
id: 0045
title: Publish the supported macOS and permission test matrix
status: done
agent: platform-engineer
model: human
release: M0
depends_on: [0002]
change: null
workstream: foundation
type: docs
priority: p0
risks: [privacy]
platform: macos
---

## Goal
Define the operating-system, architecture, filesystem, login-session, and permission combinations that each release must test or explicitly refuse.

## Acceptance criteria
- [x] The matrix names supported macOS major versions and Intel and Apple silicon expectations.
- [x] Each collector lists required, optional, and prohibited permissions with observable refusal behavior.
- [x] Annual macOS beta and release-candidate validation has an owner, evidence format, and retirement rule.

## Context
Platform support is an evidence contract, not an implication from whichever runner happened to pass.

## Notes
Implemented in `039db0e47ea943d892da78d218682c670251a55c` and retained in
`docs/evidence/0045-support-matrix.md`. The current-main continuation pipe was
rerun at `cdb04cdaa4156360b60122d23bf23566bda60d9d` on the local MacBook Pro 17,1
(Apple M1, macOS 26.6.2 arm64). Only that device row is verified; macOS 15 and
Intel remain explicit release-gate limitations and no-go/unverified entries.
