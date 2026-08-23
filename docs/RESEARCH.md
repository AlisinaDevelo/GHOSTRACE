# Research context

GHOSTRACE sits between operating-system observation, local journaling, and causal
explanation. Its differentiator is not a new claim about what macOS can observe. It
is the discipline of recording bounded, user-authorized observations with source
limits, policy context, evidence levels, and first-class gaps.

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

## Primary sources

- [Apple File System Events Programming Guide](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/)
- [Apple File System Events API](https://developer.apple.com/documentation/coreservices/file_system_events)
- [Apple Endpoint Security](https://developer.apple.com/documentation/endpointsecurity)
- [Apple Platform Security](https://support.apple.com/guide/security/welcome/web)
- [SQLite Write-Ahead Logging](https://sqlite.org/wal.html)
- [OpenTelemetry specification](https://opentelemetry.io/docs/specs/otel/)
- [W3C PROV-O](https://www.w3.org/TR/prov-o/)
- [NIST SP 800-92, Guide to Computer Security Log Management](https://csrc.nist.gov/pubs/sp/800/92/final)

The links above are source material, not endorsements or claims that GHOSTRACE
implements every referenced model.
