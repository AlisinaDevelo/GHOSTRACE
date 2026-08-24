---
id: 0046
title: Freeze semantic identifier and digest contracts
status: done
agent: security-auditor
model: human
release: M0
depends_on: [0004, 0006]
change: null
workstream: foundation
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Replace structurally valid free-form identifiers with documented semantic formats before third-party or live data is accepted.

## Acceptance criteria
- [x] Every public identifier and digest field has a type, canonical encoding, length bound, and sensitivity classification.
- [x] Constructors reject paths, credentials, control characters, and ambiguous encodings in identifier-shaped fields.
- [x] Schema, Rust validation, fixtures, and compatibility tests enforce the same contract.

## Context
This closes the gap between byte bounds and actual minimization semantics identified in the security review.

## Notes
Evidence: `docs/evidence/0046-semantic-identifier-contract.md`.
Code merged by PR #175 at `f7d3ea0dfd5817be0b31931be8a9830478179e19`; the
post-merge macOS rerun and raw log digests are recorded in the evidence report.
