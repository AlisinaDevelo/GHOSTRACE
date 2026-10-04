# Task 0103 candidate evidence: native protocol bounds

Status: candidate implementation only. This document records the local
handoff; it does not claim issue closure, a native-host release, browser
integration, or a completed fuzz campaign.

The candidate is based on `93d6c55` in branch
`fix/native-protocol-resource-bounds` and changes:

- `src/native_messaging.rs`: rejects oversized/empty bodies before semantic
  deserialization; scans JSON without building a tree; bounds nesting, values,
  object fields, duplicate/escaped keys, each decoded string, and total decoded
  string bytes; exposes connection-start deadline checks and pre-parse ingress
  admission.
- `src/native_transport.rs`: normalizes bounded Chromium stdio-shaped frames
  and a versioned, explicitly identified Safari-shaped envelope in a pure
  fixture module with no browser, socket, keychain, or journal access.
- `tests/native_messaging.rs`, `tests/native_transport.rs`, and
  `tests/fixtures/native-transport-v1.json`: regression and differential
  coverage, including rejected-frame rate accounting and a hello deadline.

## Acceptance mapping

| Criterion | Candidate evidence | Remaining gate |
|---|---|---|
| Hard byte, nesting, field, and allocation limits before semantic processing | Allocation-free pre-scan and bounded-frame tests | Run focused Rust tests and review native-host ingress ordering on the integration branch |
| Fail-closed versions, types, fields, replay, truncation, trailing data, and timeouts | Existing protocol tests plus new deadline/rate regressions | Verify native host closes/refuses on each error and retains no payload in diagnostics |
| Fuzzing and Chromium/Safari differential fixtures without a listener | Pure fixture corpus and seven libFuzzer targets | Run the bounded campaign and retain receipts; real Safari remains a separate packaging/device gate |

## Explicit non-claims

The fixture is not Safari support. No Safari extension was installed or
permissioned, and no browser runtime was exercised. No native host process was
run in this handoff. Rust/Cargo and libFuzzer execution are intentionally
pending the serialized resource authorization; therefore this file contains
no test counts, crash-free claim, memory receipt, or CI result.

The Chromium caller-origin shape follows the browser-supplied argument in the
[Chrome Native Messaging documentation](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging).
The Safari-shaped envelope is deliberately not presented as Apple's runtime
contract; that contract still needs a separate review against [Safari Web
Extension app-extension messaging](https://developer.apple.com/documentation/safariservices/messaging-between-the-app-and-javascript-in-a-safari-web-extension)
and [Safari Web Extension
packaging](https://developer.apple.com/documentation/safariservices/running-your-safari-web-extension).

The native-host owner must call `ProtocolSession::admit_attempt` for every
complete frame before parse/pairing/MAC work, then call
`receive_after_admission` only after hello pairing or message-MAC verification.
That method performs the bounded typed parse and protocol-state transition;
keeping the admission call first prevents malformed or unauthenticated traffic
from bypassing the rate budget.
