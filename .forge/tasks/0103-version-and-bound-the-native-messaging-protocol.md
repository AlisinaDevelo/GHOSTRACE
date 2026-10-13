---
id: 0103
title: Version and bound the native-messaging protocol
status: done
agent: api-designer
model: human
release: M5
parent: 0030
depends_on: [0102]
change: pr-434
workstream: browser
type: feature
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Define strict framing, message types, sequence, size, timeout, rate, version, and shutdown behavior between extension and host.

## Acceptance criteria
- [x] Length prefixes and JSON payloads are parsed with hard byte, nesting, field, and allocation limits before semantic processing.
- [x] Unknown versions, types, fields, duplicate sequence numbers, truncated frames, trailing data, and timeouts fail closed.
- [x] Fuzzing and differential fixtures cover Chromium and Safari transport adapters without opening a network listener.

## Context
Content-script input is untrusted and must not directly trigger privileged native behavior.

## Notes
Completed on reviewed protected-main source
`ee4216c8fd3ef993538655ac672aa2d37825f6d0`, after PR #434 merged. Retained
criteria `E-0103-01` through `E-0103-03` map to R-NATIVE-01/02 and
R-FUZZ-60-M5-ee4216c in the
[native completion evidence](../../docs/evidence/0030-native-host-completion.md).
R-DELIVERY-01 records independent review, passing hosted checks and merge binding.

The allocation-free JSON scan and stdio ingress admission enforce bounds before
semantic processing. Unknown versions/types/fields, sequence replay, malformed
frames and deadline/shutdown failures are covered on the actual host executable.
The exact clean merged source passed 102 distinct selected native checks on
Apple M5/macOS 27.0.1 with Rust 1.88.0. Its pinned-nightly full fuzz campaign
passed all seven targets: 18,025,804 executions, peak reported RSS 168 MB,
11 seed replays, no failure artifacts and unchanged inputs.

Chromium/Safari differential adapters remain pure transport fixtures without a
network listener; no installed browser or live Safari runtime is claimed.
The [historical protocol handoff](../../docs/evidence/0103-native-protocol-bounds-candidate.md)
retains its original source and limitations. Full production resource/fault
coverage remains task 0109; service capabilities remain task 0112.
