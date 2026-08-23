---
id: 0038
title: Harden release signing, notarization, SBOM, and dependencies
status: backlog
agent: maintainer
model: human
release: M6
depends_on: [0005, 0008, 0023, 0031, 0037, 0047, 0048, 0115, 0116, 0117, 0118]
change: null
workstream: release-scale
type: feature
priority: p1
risks: [security]
platform: macos
---

## Goal
Establish a verifiable macOS release pipeline and dependency policy suitable for distributing a privacy-sensitive local application.

## Acceptance criteria
- [ ] Locked reproducible builds cover documented architectures.
- [ ] Release artifacts are signed and notarized.
- [ ] An SBOM is produced.
- [ ] Dependency licenses and advisories are checked.
- [ ] Release builds contain no telemetry.

## Context
Release evidence must state exactly which checks ran and which artifacts they covered. Dependency review includes network behavior and unnecessary platform permissions.

## Notes
No implementation notes yet.
