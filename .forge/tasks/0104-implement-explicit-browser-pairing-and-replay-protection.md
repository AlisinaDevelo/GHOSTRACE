---
id: 0104
title: Implement explicit browser pairing and replay protection
status: done
agent: security-auditor
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
Bind a user-approved extension installation and native host to a revocable local channel without treating manifest presence as sufficient consent.

## Acceptance criteria
- [x] Pairing shows browser, profile class, extension identity, requested event classes, retained fields, and private-context policy.
- [x] Session and message nonces, expiry, monotonic sequence, restart, revocation, and extension-update behavior are specified and tested.
- [x] Copied manifests, replaced extensions, replayed transcripts, and stale approvals require refusal or re-pairing.

## Context
Registration proves browser authorization to launch a host, not the user's current consent to retain events.

## Notes
Completed on reviewed protected-main source
`ee4216c8fd3ef993538655ac672aa2d37825f6d0`, after PR #434 merged. Retained
criteria `E-0104-01` through `E-0104-03` map to R-NATIVE-01/02 and
R-PAIRING-01 in the
[native completion evidence](../../docs/evidence/0030-native-host-completion.md).
R-DELIVERY-01 records independent review, passing hosted checks and merge binding.
PR #430 and [ADR 0007](../../docs/adr/0007-native-reconnect-recovery.md) specify
stop-and-wait recovery after uncertain durable delivery.

The actual pairing CLI declines by default, discloses approval facts, stores an
approved credential encrypted and redacts its secret from list output. Four
real process tests cover decline/approval/list, fresh host nonce after restart,
refusal of previous-session MACs, changed key/permission digests requiring
re-pairing, and separate-process CLI revocation. These four tests passed again
on clean merged source and are included in the 98 integration checks; four
bridge unit checks bring the distinct total to 102.

The declared browser/profile/key/permission scope is not OS-attested identity.
Manifest registration is separate from user consent; pairing/store and journal
keys remain separate. Tests use the actual native executable, private Unix IPC
and synthetic journal on Apple M5/macOS 27.0.1 with Rust 1.88.0. Installed browser
behavior, shipped service lifecycle and broader production resource gates remain
separate tasks.
