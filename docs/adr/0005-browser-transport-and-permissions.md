# ADR 0005: Browser transport, pairing, and permissions

- **Status:** Proposed (decision owned by task 0029)
- **Date:** 2026-09-27
- **Scope:** Browser integration trust boundary

## Context

Browser messages are attacker-shaped and cross into a privileged native process.
A page can shape URLs, a compromised or replaced extension can fabricate or replay
messages, and a user or installer can widen permissions or native-host access
without GHOSTRACE noticing. The threat corpus
[`fixtures/browser-threat-corpus-v1.json`](../../fixtures/browser-threat-corpus-v1.json)
fixes the expected outcome of each of these cases before any browser code ships.

## Proposed decision

- Transport is Chrome/Chromium native messaging to a GHOSTRACE native host that
  owns framing, a versioned protocol, and pairing. No local network listener,
  WebSocket, or browser-side storage of evidence.
- Every frame is length-bounded and must be valid UTF-8 JSON of a known message
  type for the negotiated version (tasks 0103).
- The native host accepts messages only from an explicitly paired extension ID
  and key. Pairing yields a per-session key; each message carries a monotonically
  increasing sequence number and MAC. Duplicates and replays are rejected, and a
  skipped sequence records a gap (task 0104).
- URLs pass through `SanitizedUrl` before any other use. Origin canonicalization
  and path minimization are task 0106; until then the retained path is an
  accepted risk below.
- Private and incognito contexts are refused before persistence (task 0108).
- Any change of extension key, protocol version, reviewed permission set, or
  native-host manifest requires re-pairing.

## Layer outcomes

| Layer | Validates | On failure |
|---|---|---|
| `native_host_framing` | length prefix, frame bound, UTF-8 JSON | reject |
| `protocol_validator` | message type and field set for the negotiated version | reject, or re-pair on downgrade |
| `pairing` | extension ID, session key, sequence, MAC | reject, record a gap on loss, re-pair on key change |
| `sanitized_url` | scheme, host, userinfo/query/fragment removal, byte bound | reject or canonicalize |
| `policy` | enabled source, private context, reviewed permissions | reject, re-pair on drift |
| `journal` | writer admission and backpressure | record a gap |
| `native_host_install` | manifest ownership, allowed origins, digest | re-pair |

## Accepted risks

| Risk | Description | Test | Permission | User control | Rollback |
|---|---|---|---|---|---|
| R-browser-01 | A correctly paired but compromised extension can report navigations that did not happen; pairing authenticates the sender, not the truth of its reports. | navigation-fabricated-by-compromised-extension | Extension `tabs`/`webNavigation` limited to top-level frames; no page-content permission | Unpair the extension and disable the browser source | Retention deletion of browser events for the affected interval |
| R-browser-02 | Replacing the extension under the same ID can briefly deliver messages before the key change is noticed. | extension-replacement | Pairing key bound to the extension's public key | Re-pair prompt; the source stays disabled until the user confirms | Unpair, then delete events after the last verified message |
| R-browser-03 | A user-writable native-host manifest can be edited to admit another extension origin. | native-host-manifest-replaced | Manifest lists exactly one extension origin and is digest-checked at startup | Reinstall or remove the native host from GHOSTRACE | Remove the manifest; the extension can no longer reach the host |
| R-browser-04 | Until origin canonicalization ships, retained URL paths can carry secrets, trailing-dot hosts alias origins, and private-network hosts are kept. | url-path-secret | No browser collector ships before task 0106 | Browser source is disabled by default | Retention deletion; no browser data exists before the collector ships |

## Consequences

The corpus is the acceptance contract for tasks 0102–0108: each `specified` case
names the task that must enforce it, and `tests/browser_threat_corpus.rs` fails if
an enforced case changes outcome, a referenced task does not exist, or an accepted
risk loses its test, permission, user control, or rollback path.
