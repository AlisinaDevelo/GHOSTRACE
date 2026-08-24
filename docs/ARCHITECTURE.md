# Architecture

GHOSTRACE is a local, modular Rust application with a deliberately narrow data
path. The current public slice replays synthetic JSONL fixtures. Live collection is
a separate capability and remains disabled until its policy, recovery, writer, and
encryption gates pass.

## Pipeline

~~~text
source adapter
  ├─ fixture JSONL (current)
  └─ explicitly enabled macOS source (future)
        │
        v
bounded normalization
        │
        v
      typed origin capability
        │
        v
consent + capture policy (deny by default)
        │
        v
versioned event envelope + provenance
        │
        v
bounded single-writer SQLite WAL journal
        │
        ├─ deterministic query
        ├─ evidence-backed explanation
        └─ explicit JSONL export
~~~

The arrows are trust and ownership boundaries. A source does not write directly to
the journal. Policy runs before persistence. A writer acknowledges an accepted event
only after its transaction commits. If the source cannot prove coverage, it emits a
gap or an unknown evidence level rather than allowing the explanation layer to infer
one.

## Policy-document boundary

Capture policy is stored as a strict `policy-document-v1` document with an immutable
identity and a monotonically increasing policy version. The JSON document has no
extension fields: unknown schema versions, unknown fields, duplicate entries, and
invalid identifiers are rejected before a candidate reaches a journal or policy
history. A version upgrade that preserves enabled sources, selected roots, and
private-context behavior is automatically interpretable. Any semantic change must
be explicitly reconfirmed; a failed migration leaves the previously accepted
document active and retains no candidate observation.

Consent is a separate append-only state machine over that document. Each grant, scope
change, suspension, revocation, or deletion-intent transition emits a bounded receipt
with policy identity/version, a scope digest, timestamp, actor code, and reason code.
The active gate is false for every state except `active`; revocation is applied before
asynchronous cleanup, and replay rejects gaps, out-of-order receipts, mismatched
policy context, and non-grant attempts to reactivate collection.

The policy gate also emits a bounded decision record. Its finite outcomes are allow,
deny, redact, summarize, and refuse; its diagnostics distinguish policy denial,
malformed input, unsupported scope, and internal failure. The record reports only
source, policy identity/version, root presence, private-context state, and a stable
reason code. Rejected roots and observations never enter the record or its debug
representation.

## Components

| Component | Responsibility | Current state |
| --- | --- | --- |
| CLI | Parse commands, print structured results, and surface refusal reasons | Fixture commands available; capture refuses |
| Fixture adapter | Read synthetic JSONL and validate the event contract | Available |
| Source adapters | Translate bounded platform observations into the envelope | Live adapters not shipped |
| Policy gate | Apply consent, selected scope, exclusions, private-context rules, and redaction | Required before live capture |
| Event envelope | Preserve source facts, provenance, evidence level, and schema version | Versioned contract is documented; journal ingestion requires an origin capability |
| Ingest writer | Bound memory, serialize writes, and commit event plus cursor atomically | Fixture path is the current exercise; live gate remains |
| Journal | Store local event metadata and encrypted payloads when the production key path exists | SQLite/WAL design documented; Keychain production path not shipped |
| Explain/export | Produce deterministic evidence-linked explanations and explicit exports | Fixture surface available |

## Event lifecycle

1. A source produces an observation or a fixture supplies one.
2. Normalization rejects malformed or out-of-contract data without retaining
   sensitive rejected values.
3. The policy gate loads a validated, versioned policy document and decides whether
   the observation is allowed, denied, or converted to a gap/status record. Policy
   decisions have a version and reason; semantic policy upgrades require explicit
   reconfirmation.
4. An accepted event receives a stable identifier and provenance, including the
   immutable policy profile ID and version. A typed adapter-origin capability owns
   the provenance version and collector namespace; the envelope retains what the
   source actually established, not what a caller wished it had established.
5. The bounded writer persists the event and source cursor in one transaction when
   the live path is enabled. Queue pressure and unrecoverable source history become
   visible gaps.
6. Query, explanation, and export read committed records. They never mutate source
   history and never silently repair a gap.

## Storage boundary

The active journal is planned as one local SQLite database in WAL mode with one
writer and read-only readers. WAL improves reader/writer concurrency, but it is not
an encryption boundary and it does not make a source complete. SQLite metadata,
temporary files, backups, and operating-system filesystem behavior remain part of
the threat model. See [ADR 0003](adr/0003-sqlite-wal-active-journal.md).

Production sensitive payloads require authenticated encryption with a macOS Keychain
backing key. That key path is not represented as shipped live-capture capability in
the current headstart. Missing keys or failed authentication must fail closed.

The macOS key provider uses only the data-protection Keychain generic-password path:
non-synchronizable items, `WhenUnlockedThisDeviceOnly` access control, and an explicit
service/account identity. The default app has no access-group entitlement; a signed
helper may use one only when its bundle entitlement matches. Login-session availability
and the data-protection requirement are checked before returning key bytes, so an
unsigned CLI, locked session, duplicate item, or malformed item fails closed without
falling back to the legacy file keychain.

### Persistent path boundary

The fixture file-backed journal now exercises the production path boundary. Its
containing directory is created one component at a time with mode `0700`; existing
components are checked for directory type and current-user ownership, and an
attacker-controlled symlink, parent replacement, or `..` traversal is refused. The
database and SQLite sidecars (`-wal`, `-shm`, rollback journal, temporary, and backup
artifacts) must be regular, current-user-owned, single-link files with mode `0600`.
The database open uses no-follow flags plus identity checks before and after SQLite
opens the path. Every committed file-backed ingest rechecks the database and any
sidecars, so a mode or inode change becomes a bounded error rather than an implicit
write to a different location. Exports use the same regular-file, ownership, link,
and `0600` contract; a forced export can repair the mode of a single-link regular
file but never replaces a symlink or hard link.

## Explanation boundary

The explanation layer is deterministic and evidence-linked. It may describe a
supported sequence such as “an observed change followed another observed change
within the fixture window,” but it cannot turn temporal order into proof of intent or
complete causality. Every claim must identify the event IDs and evidence levels it
uses. Every uncovered interval, coalesced source result, denied observation, or
restart discontinuity is visible as a gap or limitation.

## Extension rules

A new source or output is acceptable only when it:

- has an explicit consent and scope model;
- defines fields that are retained and fields that are forbidden;
- can bound memory, payload size, and processing time;
- persists progress atomically or reports exactly what cannot be recovered;
- handles private contexts and exclusions before persistence;
- adds deterministic fixtures and failure tests;
- updates [PRIVACY.md](PRIVACY.md), [THREAT_MODEL.md](THREAT_MODEL.md), and an ADR
  when the trust boundary changes.

## Failure behavior

GHOSTRACE prefers a visible refusal or gap to an apparently complete but misleading
journal. A source that cannot reconnect to its history must not resume as if no
interval were lost. A full queue must produce backpressure or an explicit loss
record. A failed decrypt, malformed fixture, invalid policy, or existing export
destination must produce a bounded error without dumping sensitive payloads.
