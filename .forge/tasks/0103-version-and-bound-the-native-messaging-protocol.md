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

Candidate implementation on `fix/native-protocol-resource-bounds` (based on
`93d6c55`) adds the allocation-free JSON structure pre-scan and explicit field,
duplicate-key, per-string, and total-string bounds in `src/native_messaging.rs`.
It also adds the explicit-start deadline check and an ingress admission seam so
the native host can count malformed, unauthenticated, and rejected frames before
parsing or pairing. `src/native_transport.rs` and
`tests/fixtures/native-transport-v1.json` are pure Chromium/Safari-shaped
normalization fixtures only: they do not install, launch, or claim a Safari
extension adapter.

This note is not closure evidence. The native-host owner must integrate
`ProtocolSession::new_at`, `admit_attempt`, and
`receive_after_admission` around stdio reads, pairing, and MAC verification.
The focused Rust tests and the libFuzzer targets still require the serialized
toolchain/build and campaign gates; a fixture or fuzz-crate build alone does
not satisfy this task.
