---
id: 0137
title: Build the adapter conformance and fault suite
status: backlog
agent: test-engineer
model: human
release: M8
parent: 0132
depends_on: [0136]
change: null
workstream: interoperability
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Test adapters against origin, policy, schema, bounds, cursor, replay, gap, cancellation, diagnostic, and resource contracts before they can be enabled.

## Acceptance criteria
- [ ] A deterministic fake journal and source harness covers normal, malformed, hostile, slow, duplicated, reordered, dropped, and crashed adapter behavior.
- [ ] Certification evidence binds adapter version, manifest digest, test corpus, platform, results, and known exceptions.
- [ ] Runtime admission refuses adapters whose manifest or binary identity differs from the certified evidence.

## Context
A stable interface without conformance evidence would distribute privacy and correctness failures to every adapter.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
