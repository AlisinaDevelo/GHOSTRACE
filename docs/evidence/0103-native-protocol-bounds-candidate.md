# Task 0103 candidate evidence: native protocol bounds

Status: source-scoped candidate history with later merged evidence. This page
does not claim issue closure, a native-host release or installed browser support.
Combined merged-source native/pairing reproduction is retained in the
[current completion evidence](0030-native-host-completion.md).

## Historical candidate

The original handoff was based on `93d6c55` in branch
`fix/native-protocol-resource-bounds` and changed:

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

## Later merged evidence

Native stdio ingress is integrated. Authenticated reconnect recovery merged in
[PR #430](https://github.com/AlisinaDevelo/GHOSTRACE/pull/430) at
`f3fe7c1a7fa9c9f8b6124cdfca38532ecc5cb5ed`.
[PR #433](https://github.com/AlisinaDevelo/GHOSTRACE/pull/433) merged the bounded
runner and continuous CI at `1f78a3aa92eaee63fdd7012bf69d0b978499ccc6`.
Its clean M5 60-second-per-target campaign passed all seven targets with
8,417,188 executions, peak reported RSS 139 MB, no failure artifacts and unchanged
inputs. Receipt `R-FUZZ-60-M5-1f78a3a/receipt.json` has SHA-256
`1612b3bb0fd0dc62ed7bb002c7f8775964bf959c172e09923b6728535d5372c4`.

[PR #431](https://github.com/AlisinaDevelo/GHOSTRACE/pull/431), merged
`b2f86a20bd7ee3b8e02dc98237a2218a1e1dd8f8`, aligned action pins and suppressed
Python bytecode. Clean hosted artifact 11689810518 passed all seven targets at
`e886e997145446b5b94758875d3912da4d847b91`. These Linux results are supporting CI
evidence, separate from the M5 observations and merged-source native/fixture receipts.

## Acceptance mapping

| Criterion ID | Implemented evidence | Current proof gate |
|---|---|---|
| E-0103-01: hard byte, nesting, field and allocation limits before semantics | Allocation-free pre-scan, bounded-frame tests and integrated ingress admission | R-NATIVE-01/02 at clean merged `ee4216c`; see the current completion evidence |
| E-0103-02: fail-closed versions, types, fields, replay, truncation, trailing data and timeouts | Protocol and actual stdio refusal/deadline/shutdown tests, fixed diagnostics | R-NATIVE-01/02 at clean merged `ee4216c` |
| E-0103-03: fuzz and Chromium/Safari differential fixtures without a listener | Pure differential corpus, seven targets, observed clean M5 full campaign | R-NATIVE-01 and R-FUZZ-60-M5-ee4216c; live Safari remains separate |

## Explicit non-claims

No native-host process, Rust/Cargo test or libFuzzer campaign ran in the original
`93d6c55` resource-constrained handoff. That historical limitation does not
describe the later merged integration and observed campaign above. The fixture
is not Safari support: no Safari extension was installed or permissioned, and
no browser runtime was exercised by these fixtures. Actual native executable
and synthetic-journal tests do not prove installed browser operation.

The Chromium caller-origin shape follows the browser-supplied argument in the
[Chrome Native Messaging documentation](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging).
The Safari-shaped envelope is deliberately not presented as Apple's runtime
contract; that contract still needs a separate review against [Safari Web
Extension app-extension messaging](https://developer.apple.com/documentation/safariservices/messaging-between-the-app-and-javascript-in-a-safari-web-extension)
and [Safari Web Extension
packaging](https://developer.apple.com/documentation/safariservices/running-your-safari-web-extension).

The integrated native host calls `ProtocolSession::admit_attempt` for every
complete frame before parse/pairing/MAC work, then calls
`receive_after_admission` only after hello pairing or message-MAC verification.
That method performs the bounded typed parse and protocol-state transition;
keeping the admission call first prevents malformed or unauthenticated traffic
from bypassing the rate budget.
