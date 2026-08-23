---
id: 0003
title: Scaffold the MPL-2.0 Rust core and CI
status: done
agent: maintainer
model: human
release: M0
depends_on: [0001]
change: null
workstream: foundation
type: feature
priority: p0
risks: []
platform: any
---

## Goal
Create the minimal Rust repository and continuous-integration baseline for a versioned, testable command-line application.

## Acceptance criteria
- [x] The repository contains one Cargo package and the planned module skeleton.
- [x] The MPL-2.0 license and required community files are present.
- [x] The executable supports --help and --version.
- [x] Locked builds and format, Clippy, and test checks run in CI.

## Context
Begin with a modular monolith. Live collectors, a desktop interface, and background collection remain outside this task.

## Notes
The 0.0.1 modular Rust package, MPL-2.0 community surface, SHA-pinned workflows, and help/version smoke tests are checked in. Live collectors and UI remain absent.
