---
id: 0007
title: Implement the consent and capture-policy engine
status: done
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
- [x] Consent is deny-by-default.
- [x] Selected roots, exclusions, private context, and redaction decisions are enforced.
- [x] Decisions are versioned and reason-coded.
- [x] Property tests cover the consent and policy state machine.

## Context
No event may be persisted before policy evaluation. Blocked sensitive values must not be retained merely for diagnostics.

## Notes
The policy gate now carries an independent bounded exclusion set. Exclusions are
validated, included in the scope digest, represented by the stable `root_excluded`
reason, and treated as a semantic migration that requires reconfirmation. Older v1
documents may omit the optional field and receive an empty set. The runtime profile
and journal authorization path validate the same constraints before allowing a
record. A deterministic dependency-free corpus covers 512 policy matrices and 256
consent command sequences, including redaction, replay, failed-command immutability,
and rejection of forged non-grant reactivation.

Target-device verification was run on Apple M1 macOS 26.6.2 with Rust/Cargo 1.88.0;
the complete debug, release, enforced sandbox, static, and property receipts are
listed in `docs/evidence/0007-policy-consent.md`.
