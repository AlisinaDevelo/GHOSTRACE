---
id: 0007
title: Implement the consent and capture-policy engine
status: backlog
agent: maintainer
model: human
release: M1
depends_on: [0004, 0006, 0051, 0052, 0053]
change: null
workstream: privacy
type: feature
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Create the hard policy gate that decides whether and how an observed event may enter the journal.

## Acceptance criteria
- [ ] Consent is deny-by-default.
- [ ] Selected roots, exclusions, private context, and redaction decisions are enforced.
- [ ] Decisions are versioned and reason-coded.
- [ ] Property tests cover the consent and policy state machine.

## Context
No event may be persisted before policy evaluation. Blocked sensitive values must not be retained merely for diagnostics.

## Notes
The fixture foundation is deny-by-default, selected-root aware, private-context aware, versioned, and reason-coded. The journal requires the matching policy at its persistence boundary, and a policy ID/version is immutable. Exclusions, explicit redaction outcomes, and property-based state-machine coverage remain open.
