---
id: 0001
title: Publish GHOSTRACE product contract and non-goals
status: done
agent: maintainer
model: human
release: M0
depends_on: []
change: null
workstream: foundation
type: docs
priority: p0
risks: [privacy]
platform: any
---

## Goal
Publish the product contract that defines GHOSTRACE as a local-only, opt-in causal event journal and fixes its permanent privacy boundaries without implying legal-forensic guarantees.

## Acceptance criteria
- [x] The README states local-only behavior and opt-in collectors.
- [x] The README lists prohibited sensors and the private-browsing default.
- [x] The README documents the upload policy and FSEvents completeness limitations.

## Context
This contract is the foundation for every collector, storage, and release decision. It must make limitations as visible as capabilities.

## Notes
Verified in the M0 README, privacy model, and local-only ADR. Live capture remains an explicit refusal.
