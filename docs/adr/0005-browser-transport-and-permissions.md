# ADR 0005: Browser transport, pairing, and permissions

- **Status:** Accepted (task 0029)
- **Date:** 2026-09-27
- **Scope:** Browser integration trust boundary

## Context

Browser messages are attacker-shaped and cross into a privileged native process.
A page can shape URLs, a compromised or replaced extension can fabricate or replay
messages, and a user or installer can widen permissions or native-host access
without GHOSTRACE noticing. The threat corpus
[`fixtures/browser-threat-corpus-v1.json`](../../fixtures/browser-threat-corpus-v1.json)
fixes the expected outcome of each of these cases before any browser code ships.

## Decision

- Transport has two hops and no network listener. The extension talks to a
  GHOSTRACE native host through Chrome/Chromium native messaging (stdin/stdout of a
  host process the browser launches). The native host relays accepted, already
  canonicalized records to the GHOSTRACE local service over a Unix-domain socket
  whose directory is owned by the user with mode 0700 and whose socket is mode
  0600 (tasks 0035 and 0111). The native host never writes the journal itself; the
  service remains the single writer.
- **Localhost HTTP is rejected.** No TCP listener on loopback or any other
  interface, no WebSocket, no local HTTP server, and no `externally_connectable`
  web origin. A loopback port is reachable by every local process and by web
  pages through DNS rebinding, and it gives no peer identity.
- Message limits (task 0103, `src/native_messaging.rs`): inbound frames of at
  most 64 KiB with a native-endian 4-byte length prefix, UTF-8 JSON with at most 8
  levels of nesting and 256 values, four v1 message types (`hello`, `navigation`,
  `heartbeat`, `goodbye`) with unknown types and fields refused, a 120-second idle
  deadline, at most 200 messages per 10-second window, and nothing accepted after
  `goodbye`.
- Extension allowlist: the native-host manifest's `allowed_origins` lists exactly
  one `chrome-extension://<id>/` origin per supported browser channel. Wildcards
  and unknown origins are refused at install (task 0102), and the manifest digest
  is checked at startup.
- Pairing: manifest presence is not consent. The user approves a pairing that
  shows browser, profile class, extension identity, event classes, retained
  fields, and the private-context policy. Pairing binds the extension's public key
  and yields a per-session key; each message carries a strictly increasing
  sequence number and MAC. Duplicates and replays are rejected, a skipped sequence
  records a gap, and a changed key, protocol version, permission set, or manifest
  requires re-pairing (task 0104).
- Minimum extension permissions: `nativeMessaging` and `webNavigation` only, with
  top-level frames used. No host permissions, `<all_urls>`, `tabs` content access,
  `history`, `cookies`, `webRequest`, `scripting`, content scripts, or
  `externally_connectable`. Bookmark collection, if enabled, adds only `bookmarks`
  (task 0107). Any broader permission is a permission-manifest change (task 0115).
- Private-context policy: the extension manifest sets `incognito: "not_allowed"`,
  so it never runs in private windows. As a second barrier every navigation
  carries `private_context`, and `CanonicalNavigation::from_url` refuses a private
  navigation before parsing its URL (task 0106); the refusal is counted in a
  policy-blocked summary and nothing about the page is kept (task 0108).
- URLs are reduced by `CanonicalNavigation` to a bounded origin (or an opt-in
  first-path-segment class) before anything else sees them (task 0106).
- Any change of extension key, protocol version, reviewed permission set, or
  native-host manifest requires re-pairing.

## Alternatives considered

1. **Localhost HTTP or WebSocket server:** rejected; reachable by any local process
   and by web pages through DNS rebinding, with no peer identity.
2. **Extension writes a file the service watches:** rejected; extensions cannot
   write arbitrary files, and a shared file has no ordering, backpressure, or
   authentication.
3. **Native host writes the journal directly:** rejected; it would create a second
   writer and give a browser-launched process journal key access.
4. **Safari WebExtension with an app extension handler:** deferred to task 0034;
   it ships only if it can meet the same limits and pairing.

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
