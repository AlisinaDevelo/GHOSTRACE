---
id: 0103
title: Version and bound the native-messaging protocol
status: backlog
agent: api-designer
model: human
release: M5
parent: 0030
depends_on: [0102]
change: null
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define strict framing, message types, sequence, size, timeout, rate, version, and shutdown behavior between extension and host.

## Acceptance criteria
- [ ] Length prefixes and JSON payloads are parsed with hard byte, nesting, field, and allocation limits before semantic processing.
- [ ] Unknown versions, types, fields, duplicate sequence numbers, truncated frames, trailing data, and timeouts fail closed.
- [ ] Fuzzing and differential fixtures cover Chromium and Safari transport adapters without opening a network listener.

## Context
Content-script input is untrusted and must not directly trigger privileged native behavior.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
