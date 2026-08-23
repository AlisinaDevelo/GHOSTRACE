---
id: 0002
title: Record macOS support, permission, and collector boundaries
status: done
agent: maintainer
model: human
release: M0
depends_on: [0001]
change: null
workstream: foundation
type: docs
priority: p0
risks: []
platform: macos
---

## Goal
Record the supported macOS platforms, permission model, and collector boundaries before platform-specific code is introduced.

## Acceptance criteria
- [x] The ADR documents the supported macOS floor and processor architectures.
- [x] The ADR documents required TCC permissions and the no-root policy.
- [x] The ADR documents why Endpoint Security is deferred from the baseline product.

## Context
The decision must cover Intel versus Apple silicon support and distinguish required platform access from capabilities the product deliberately excludes.

## Notes
The platform policy sets macOS 15.0 as the development floor, targets Apple silicon and Intel, and requires release-time revalidation. ADR 0002 records the FSEvents-first decision.
