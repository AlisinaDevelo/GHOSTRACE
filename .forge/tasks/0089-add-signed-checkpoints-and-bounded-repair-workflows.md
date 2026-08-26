---
id: 0089
title: Add signed checkpoints and bounded repair workflows
status: done
agent: incident-responder
model: human
release: M3
parent: 0023
depends_on: [0088]
change: pr-310
workstream: explain-export
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Create local verification checkpoints and an explicit repair mode that preserves corruption evidence instead of silently rewriting history.

## Acceptance criteria
- [x] Checkpoints bind chain position, database identity, schema, key generation, policy set, and verification time.
- [x] Repair operates on a verified copy, emits before-and-after manifests, and records every dropped or reconstructed interval as a gap.
- [x] The normal writer refuses a journal with unresolved integrity failures.

## Context
Tamper detection needs an operational response that separates recovery from normal ingestion.

## Notes
Implemented in PR #310 and merged to protected `main` at
`799a1ff3c787bfd64c57d9e3c0d2b1aa3a951978`. Completion evidence is retained in
`docs/evidence/0089-signed-checkpoints-repair.md`; issue closure alone is not
evidence.
