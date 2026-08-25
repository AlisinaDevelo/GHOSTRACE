# Research context

GHOSTRACE sits between operating-system observation, local journaling, and
evidence-linked change explanation. Its differentiator is not a new claim about what
macOS can observe. It is the discipline of recording bounded, user-authorized
observations with source limits, policy context, evidence levels, and first-class
gaps.

The project name also has a research namespace caveat. VUSec uses GhostRace for
speculative race-condition research; GHOSTRACE is unrelated. See the
[preliminary identity audit](IDENTITY.md) before using the name in a release,
package registry, or distribution listing.

## Current landscape

### Adjacent products and tools

GHOSTRACE overlaps several established categories without replacing their primary
jobs:

| System | Primary job | Boundary relevant to GHOSTRACE |
| --- | --- | --- |
| [ActivityWatch](https://activitywatch.net/) | Local, cross-platform time and activity tracking | Tracks applications, window titles, and browser activity; GHOSTRACE instead excludes window titles and treats bounded change evidence—not time allocation—as the product |
| [screenpipe](https://screenpipe.com/) | Searchable computer history for agent context | Captures screen, audio, application, and accessibility context; those sensors are permanent baseline non-goals for GHOSTRACE |
| [Timing](https://timingapp.com/) | Automatic macOS time tracking | Optimizes for work-duration attribution; GHOSTRACE optimizes for source-cited change reconstruction and explicit coverage gaps |
| [Objective-See FileMonitor and ProcessMonitor](https://objective-see.org/products/utilities.html) | Detailed macOS file and process event observation | Demonstrate the attribution available through Endpoint Security and its broader permission footprint; GHOSTRACE defers that mode behind a separate decision |
| [Santa](https://github.com/northpolesec/santa) | macOS binary and file-access authorization | A defense and policy system with system-wide monitoring; GHOSTRACE is a personal explanatory journal and is not an execution-control product |

This comparison is about product boundaries, not a claim of feature parity. The
credible gap is a user-controlled journal that can answer *what changed and which
observations support the link* while keeping collection narrow and disclosing when
the available sources cannot answer.

### macOS event sources

Apple's File System Events API is designed to notify clients about filesystem
changes. It is useful for selected-root metadata, but notifications can be
coalesced and do not by themselves provide process attribution or complete causal
history. GHOSTRACE therefore treats FSEvents as a source with explicit coverage
limits.

Apple's Endpoint Security framework provides richer security-event observation, but
its use is entitlement-gated and has a materially broader trust and operational
surface. GHOSTRACE keeps it optional and deferred until a separate policy,
permission, and attribution decision is evidenced.

### Logs and traces

OpenTelemetry standardizes telemetry signals and context propagation for
applications and infrastructure. It is valuable for service traces and logs, but a
local macOS journal has a different boundary: no default network exporter, no
ambient application-content capture, and an explicit explanation of source gaps.
GHOSTRACE can borrow ideas such as stable context and explicit provenance without
becoming a telemetry agent.

### Provenance and audit records

W3C PROV provides a vocabulary for entities, activities, and agents and is a useful
reference for naming provenance relationships. NIST log-management guidance
emphasizes collection, storage, analysis, and retention controls. GHOSTRACE narrows
those ideas to a user-controlled local journal and refuses to present the result as
legal evidence or an exhaustive audit log.

### Local storage

SQLite WAL is a practical local active-journal mechanism because it supports one
writer with concurrent readers and transactional recovery. WAL is not encryption,
does not remove side-channel metadata, and does not repair incomplete sources.
GHOSTRACE pairs it with bounded writes, explicit gaps, and a future Keychain-backed
payload key.

### Future platform contracts

Later integrations remain design work, not shipped capability. Their ADRs and test
matrices must begin with these platform constraints:

- A per-user background helper should use Apple's Service Management lifecycle and
  preserve explicit approval, status, registration, unregistration, and denial
  outcomes. A root launch daemon is outside the baseline.
- Frontmost-application context should use `NSWorkspace` activation and session
  notifications and retain only the bounded application identity in the event
  schema. Accessibility trust is a separate permission boundary, not an implicit
  fallback for titles, documents, or UI contents.
- Safari support must pass a go/no-go gate against Apple's packaging, signing,
  private-browsing, profile, website-permission, and native-messaging behavior. A
  documented no-go is preferable to broader permissions or a network listener.
- Chromium navigation and bookmark sources require distinct declared permissions.
  Incognito access is user-controlled and must still be refused by GHOSTRACE even
  when the browser says the extension is allowed to run there.
- A Tauri UI must expose only explicitly named commands, capabilities, windows, and
  scopes. Remote API access stays disabled, and command implementations must enforce
  their scopes rather than treating configuration as sufficient validation.
- A local Unix-domain service must authenticate the connected peer with the macOS
  credential interface, validate file ownership and mode, and keep TCP and Bonjour
  outside the baseline. Socket location alone is not client authentication.

## Differentiation

GHOSTRACE makes four choices explicit:

1. **Evidence before narrative.** Explanations cite event IDs and evidence levels;
   they cannot turn missing observations into a story.
2. **Capture is a capability, not the default.** The first release is fixture-only,
   local, offline, and capture-disabled while privacy gates are built.
3. **Gaps are data.** Coalescing, denial, restart loss, source errors, and queue
   drops are represented so coverage is inspectable.
4. **Scope is user-facing.** Selected roots, private-context behavior, redaction,
   export, and deletion are part of the product contract rather than hidden
   collector flags.

These choices distinguish GHOSTRACE from generic file watchers, remote observability
agents, browser-history products, and process monitors without claiming that those
systems solve the same problem.

## Research questions

- Which FSEvents flags and cursor transitions predict a safe replay boundary?
- What bounded metadata is useful for change explanation without exposing content?
- How should optional Endpoint Security attribution be compared against its
  entitlement and permission cost?
- Which gap taxonomy remains understandable across filesystem, shell, Git, and
  browser-shaped sources?
- What local key and export lifecycle is usable without creating a broader account
  or cloud dependency?
- Which claim grammar and abstention rules minimize unsupported causal conclusions
  across lossy, disabled, denied, and conflicting sources?
- Can people correctly distinguish direct evidence, context, inference, conflicts,
  retention gaps, and collection gaps in realistic investigation tasks?
- How much privacy leakage remains in operational residue such as WAL sidecars,
  diagnostics, support bundles, backups, archives, and crash paths?
- Which conservative W3C PROV and offline OpenTelemetry mappings preserve source
  limitations instead of strengthening imported or exported claims?
- Can a third-party adapter model enforce declared capabilities, origin, bounds,
  cursor semantics, and gap behavior without granting arbitrary journal authority?
- What five-year energy, storage, compatibility, and maintenance cost is acceptable
  for a continuously running local journal on supported macOS hardware?

## Primary sources

- [Apple File System Events Programming Guide](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/)
- [Apple File System Events API](https://developer.apple.com/documentation/coreservices/file_system_events)
- [Apple Endpoint Security](https://developer.apple.com/documentation/endpointsecurity)
- [Apple Platform Security](https://support.apple.com/guide/security/welcome/web)
- [SQLite Write-Ahead Logging](https://sqlite.org/wal.html)
- [OpenTelemetry specification](https://opentelemetry.io/docs/specs/otel/)
- [W3C PROV-O](https://www.w3.org/TR/prov-o/)
- [NIST SP 800-92, Guide to Computer Security Log Management](https://csrc.nist.gov/pubs/sp/800/92/final)
- [Apple TN3137, On Mac keychain APIs and implementations](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
- [Apple Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Chrome Native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)
- [Chrome `webNavigation` API](https://developer.chrome.com/docs/extensions/reference/api/webNavigation)
- [Chrome `bookmarks` API](https://developer.chrome.com/docs/extensions/reference/api/bookmarks)
- [Chrome extension permissions and incognito access](https://developer.chrome.com/docs/extensions/develop/concepts/declare-permissions)
- [Chrome guidance on extension user privacy](https://developer.chrome.com/docs/extensions/develop/security-privacy/user-privacy)
- [Apple Service Management](https://developer.apple.com/documentation/servicemanagement)
- [Apple `SMAppService.register()`](https://developer.apple.com/documentation/servicemanagement/smappservice/register%28%29)
- [Apple `NSWorkspace.didActivateApplicationNotification`](https://developer.apple.com/documentation/appkit/nsworkspace/didactivateapplicationnotification)
- [Apple `AXIsProcessTrustedWithOptions`](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)
- [Apple Safari Web Extensions](https://developer.apple.com/documentation/safariservices/safari-web-extensions)
- [Apple Safari Web Extension permissions](https://developer.apple.com/documentation/safariservices/managing-safari-web-extension-permissions)
- [Apple `getpeereid(3)` manual page](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/getpeereid.3.html)
- [Tauri 2 capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri 2 runtime authority](https://v2.tauri.app/security/runtime-authority/)
- [Tauri 2 command scopes](https://v2.tauri.app/security/scope/)
- [SLSA specification 1.2](https://slsa.dev/spec/v1.2/)
- [NIST SP 800-218, Secure Software Development Framework](https://csrc.nist.gov/pubs/sp/800/218/final)

The links above are source material, not endorsements or claims that GHOSTRACE
implements every referenced model.
