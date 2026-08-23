---
id: 0106
title: Canonicalize browser origins without retaining secrets
status: backlog
agent: security-auditor
model: human
release: M5
parent: 0031
depends_on: [0050, 0105]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Normalize approved navigation evidence to a bounded origin or policy-selected URL shape that excludes userinfo, query, fragment, and attacker-controlled display ambiguity.

## Acceptance criteria
- [ ] Parsing uses a standards-based URL implementation and requires a scheme-specific host policy.
- [ ] Internationalized names, ports, IPv6, file, blob, data, extension, about, invalid, and opaque URLs have explicit outcomes.
- [ ] Golden and property tests prove credentials, queries, fragments, and private-context markers never serialize.

## Context
Prefix validation is insufficient for browser URL privacy and origin security.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
