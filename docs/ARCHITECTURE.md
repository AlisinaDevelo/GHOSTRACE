# Architecture

GHOSTRACE is a local, modular Rust application with a deliberately narrow data
path. The public slice replays synthetic JSONL fixtures and now contains an explicitly
enabled selected-root FSEvents source. Ambient CLI capture remains disabled until the
remaining path-policy, recovery, and release gates pass.

## Pipeline

~~~text
source adapter
  ├─ fixture JSONL (current)
  └─ explicitly enabled selected-root FSEvents source (current API)
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

## Fixture-only CLI path

The developer-facing path is deliberately restartable and offline:

```text
init --journal <private path>
        |
        v
ingest --journal <path> --fixture <JSONL>
        |
        +--> explain --journal <path> --event <UUID>
        |
        +--> export --journal <path> --output <JSONL>
```

`init` is idempotent and runs the same hardened SQLite path checks as the library.
`ingest` reopens the durable journal in a separate process, validates the fixture
origin and deny-by-default policy, and commits the batch before reporting success.
`explain` and `export` reopen that journal with the same synthetic fixture key and
therefore exercise the persistence boundary rather than an in-memory shortcut.
The key is intentionally deterministic only for the synthetic headstart; it is not
the production Keychain design. `capture` remains an explicit refusal, and no CLI
command enables a live collector or network path.

## FSEvents lifecycle boundary

The `fsevents` module is a deliberately small native boundary beneath the selected-root
collector. `FseventsStream::new` validates a bounded path list and creates an
`FSEventStreamRef` with a boxed Rust callback context. The wrapper is `!Send` and
`!Sync`; the creating thread must schedule and drive the stream on its current
`CFRunLoop` and must perform start, flush, stop, restart, invalidate, and drop on
that same thread. Callback paths are copied into bounded `PathBuf` values and no
file is opened or read.

The adapter accepts only the Core Services raw C-string path representation;
CFType, extended-data, full-history, and document-ID callback modes are rejected
before native creation rather than being parsed as the wrong pointer type.

The context has no Core Foundation retain/release callbacks. The shutdown fence is
strict: stop a running stream, invalidate a scheduled stream, release the native
object exactly once, then reclaim the boxed callback state. Callback parsing rejects
null pointers, oversized batches, and oversized paths. User callback panics are
caught at the ABI boundary and exposed as a bounded health counter rather than
unwinding into CoreServices. The `fsevents` module itself does not decide consent,
exclusions, cursors, or journal policy; those responsibilities belong to the collector
adapter below.

### FSEvents flag normalization

`FseventsEvent::normalize_flags` converts the raw `u32` callback word into the
strict `fsevents-normalized-v1` contract. Every documented Apple event bit has a
typed enum member and remains in the canonical numeric order; the raw word and any
future bits are retained as bounded numeric evidence. Unknown bits produce an
explicit `unsupported` status and lower completeness rather than disappearing.

Loss and coverage boundaries are first-class: dropped buffers, a required subtree
scan, or wrapped event IDs produce `rescan_required`; root, mount, unmount, and
history markers produce `boundary`. File/dir and mount/unmount contradictions are
refused as `contradictory` while preserving the complete raw flag set. A normalized
record intentionally contains no path; path containment and digest policy remain
the responsibility of the selected-root collector.

## Selected-root collector boundary

`FseventsCollector::new` consumes a `ConsentConfirmation`, validates that the policy
enables both filesystem and lifecycle sources, and requires an exact opaque-root to
canonical-path mapping. Construction never starts the stream. `start` is the explicit
enable operation; it records a typed lifecycle event before native observation begins.

Callback batches are copied into a bounded owner-thread queue. The drain step normalizes
the flags, resolves the reported path through the operating system's canonicalization,
checks component containment plus device/inode identity, applies the versioned policy,
checks the bounded internal-artifact policy, and then applies the filesystem delivery
contract. The journal path and SQLite sidecars are registered automatically; callers
register export, backup, and temporary directories explicitly. Internal matches are
denied before path hashing or writer admission and produce a path-free
`internal_storage_path` policy-blocked summary. Existing internal objects remain
denied after relocation through device/inode binding, while symlink redirects fail
closed during canonicalization and selected-root containment. Exact transport duplicates are
suppressed only when their source event ID, raw flags, and path digest all match a
bounded event-ID window; the suppression count is exposed in collector status and
never becomes a missing filesystem event. Source coalescing, repeated modification,
and the source's `OwnEvent` flag remain explicit path-free qualifiers; OwnEvent is
accepted as evidence for unrelated paths rather than treated as a blanket drop rule.
A rename is recorded with an unknown old-to-new pairing unless a future bounded
adapter can provide contextual support; the collector never infers a path from
temporal adjacency.
hashes canonical path bytes inside a root-scoped `sha256:...` digest domain, and creates
only a `FilesystemChanged` payload with operation, entry kind, root ID, path class, and
digest. It never opens the reported path or reads file content. Case and Unicode
equivalence are whatever the selected filesystem resolves; the collector does not invent
cross-volume equivalence.
Accepted events and lifecycle transitions use the existing single `Writer`; queue
overflow becomes a first-class gap, and blocked observations become a bounded summary.
The callback queue is capped at `MAX_PENDING_EVENTS`, while one bounded emergency
writer reservation remains available for a loss/status record when normal work is
saturated. The public status exposes running/stopped/revoked state, callback health,
accepted and dropped counts, pending/overflow counts, writer reservations, and
coverage-loss state without retaining paths.

The storm/lifecycle evaluation contract is kept separate from the source boundary in
[`fixtures/fsevents-lifecycle-corpus-v1.json`](../fixtures/fsevents-lifecycle-corpus-v1.json).
It records ground-truth operations, expected direct observations, allowed coalescing,
required gaps, recovery gates, and resource limits. The macOS integration test executes
only private, non-disruptive rows and emits path-free counts. Sleep/wake, logout, and
volume-detach rows are guarded no-go cases: they require an explicitly authorized
interactive run and cannot be satisfied by replay or hosted CI.

The reproducible benchmark contract in
[`fixtures/filesystem-benchmark-corpus-v1.json`](../fixtures/filesystem-benchmark-corpus-v1.json)
extends this evaluation with small, deep, wide, Unicode, case-variant, Git,
build-output, and event-storm trees. The native runner reports workload-to-journal
latency percentiles, evidence classes, duplicates, gaps, CPU, resident memory,
journal growth, and power-telemetry status. A cursor regression remains a recorded
failure/gap; the runner never turns it into a completeness claim.

When a later consumer must open an existing item, `SelectedRoot::open_contained`
performs a descriptor walk from the selected root. Each component is opened with
`O_NOFOLLOW`; parent descriptors, not a revalidated pathname, authorize the next
lookup. A replaced root, symlink component, different-device descendant, or regular
file with a hard-link alias becomes an explicit refusal. The returned
`ContainedFile` exposes descriptor metadata and identity stability but does not
implement content reads, keeping source facts path-free and content-free.

Each selected root also records path-free volume evidence: device number,
filesystem identity, and an optional digest of a platform volume UUID. Mutable
volume display names are not identity fields. `CursorIdentity::for_volume` binds
live cursors to both this volume evidence and the selected per-host or per-device
stream mode; a matching path or collector instance cannot resume a cursor from a
different volume. Mount observations classify unmount, remount, device
replacement, APFS snapshot restore, and path reuse as explicit discontinuities.
The collector now persists one replay boundary per source and volume whenever a
source cursor advances. The boundary records selected-root and exclusion digests
plus `since_when`, latency, file-event mode, and stream identity. A changed
setting fails closed until an explicit reset or wrap establishes a new epoch.
Selected-root streams request `WatchRoot`; dropped, wrapped, subtree-scan, and
root-change callbacks become first-class gaps with stable reason codes, bounded
volume/root digests, cursor ranges when knowable, and a remediation action. A
source-loss gap sets `recovery_required`, so the collector does not resume
ordinary filesystem events until a later reconciliation stage clears that gate.
Startup readiness is a separate coverage state: `SinceNow` is explicitly live,
whereas a nonzero ordered cursor enters `Replaying` and may claim live delivery
only after the native `HistoryDone` sentinel is consumed as a state transition.
Zero, stale, future, wrapped, and corrupted resume positions are refused rather
than silently downgraded to `SinceNow`. A timeout, partial-history status, or
explicit stop before `HistoryDone` emits a bounded `fsevents_history_*` gap,
enters `HistoryUnavailable`, and keeps `recovery_required` set. `HistoryDone`
never becomes a `FilesystemChanged` record or a user observation.
Gap events are committed with their cursor advancement in the same transaction;
durable restart recovery and the ambient CLI remain later gates (tasks 0015–0017
and their children).

## Policy-document boundary

Capture policy is stored as a strict `policy-document-v1` document with an immutable
identity and a monotonically increasing policy version. The JSON document has no
extension fields: unknown schema versions, unknown fields, duplicate entries, and
invalid identifiers are rejected before a candidate reaches a journal or policy
history. It contains selected roots and a separate bounded exclusion set; an
exclusion always wins over a selected-root grant. A version upgrade that preserves
enabled sources, selected roots, exclusions, and private-context behavior is
automatically interpretable. Any semantic change, including an exclusion change,
must be explicitly reconfirmed; a failed migration leaves the previously accepted
document active and retains no candidate observation. The scope digest covers both
root sets so consent receipts cannot silently outlive a scope change. The optional
v1 exclusion field defaults to empty only for backwards-compatible documents;
present values are still validated for uniqueness, size, and identifier shape.

Consent is a separate append-only state machine over that document. Each grant, scope
change, suspension, revocation, or deletion-intent transition emits a bounded receipt
with policy identity/version, a scope digest, timestamp, actor code, and reason code.
The active gate is false for every state except `active`; revocation is applied before
asynchronous cleanup, and replay rejects gaps, out-of-order receipts, mismatched
policy context, and non-grant attempts to reactivate collection.

The selected-root gate exposes a bounded `ConsentPreview` before activation. It shows
canonical opaque root identities, exclusions, retained fields, and known coverage
limits; an explicit confirmation is consumed by `grant_preview` to create the
receipt. The preview is user-visible but is not copied into diagnostics or receipts,
which retain only its immutable policy identity/version and scope digest. Revocation
returns a `revoked` terminal receipt synchronously, so an adapter can stop observing
before any cleanup work runs.

The policy gate also emits a bounded decision record. Its finite outcomes are allow,
deny, redact, summarize, and refuse; its diagnostics distinguish policy denial,
malformed input, unsupported scope, and internal failure. The record reports only
source, policy identity/version, root presence, private-context state, and a stable
reason code. Rejected roots and observations never enter the record or its debug
representation. The deterministic property corpus covers the policy precedence
matrix, explicit redaction/summarization, migration reason codes, consent replay,
and rejection of silent reactivation before this gate is connected to a live
collector.

## Versioned exclusion matching

The pre-persistence exclusion engine is a separate `exclusion-policy-v1` document
with a positive version and at most 128 rules. It evaluates an ephemeral subject
(root identity, relative path, file kind, application, temporary-file flag, and VCS
flag) and returns only an action, rule class, reason code, and policy version. It
never places the observed path, application, or pattern in a decision record.

Precedence is deterministic and independent of input order:

1. Safety action: `deny` > `redact` > `summarize` > `allow`.
2. Rule class: user pattern > subtree > root > application > file kind > temporary
   file > VCS.
3. Specificity: more literal pattern content wins; an identical decision does not
   depend on which equal rule appeared first.

Subtree, application, and user patterns use a bounded glob language (`*`, `?`, and
explicit backslash escapes). Matching is greedy and linear rather than regex
backtracking. Paths reject absolute, traversal, control-character, and oversized
inputs before matching; case-folding is deterministic and no cross-volume identity
is inferred. `ExclusionPolicyHistory` applies a newly installed version only to
future subjects while retaining validated prior versions for explicitly recorded
evidence.

## Bounded durable writer contract

All adapters hand accepted batches to one FIFO `Writer`; they never open a second
SQLite write path. The default contract admits at most 64 outstanding requests, 16
events per batch, 4 MiB of serialized request memory, and a 250 ms admission wait.
SQLite busy/locked retries are capped at two (three total attempts). These limits are
part of `WriterConfig`, whose validation rejects zero, oversized, or otherwise unsafe
values before a worker starts. A source may select `Block`, `Reject`, or `EmitGap`
queue pressure behavior independently; an emitted gap contains the source and count
and is a caller-visible repair obligation, never an implicit drop.

Each queued request carries its typed origin, one source, events, policy profile, and
bounded diagnostic records. The journal commits the event rows, cursor updates,
policy reference, and diagnostics in one transaction. Only the successful return
from that transaction produces `WriteAck`, including request ID, event IDs, ingest
sequences, attempt count, and commit time. A request cancelled before the worker
starts is reported as `WriterCancelled`; once the transaction starts, cancellation
cannot make a committed write disappear. Queue, memory, cancellation, busy-retry,
and acknowledgement-timeout paths are covered by deterministic tests. Diagnostic
codes are short ASCII identifiers and details are limited to 512 non-control bytes;
payloads and paths are not accepted as diagnostic text.

## Cursor contract

Cursor state is part of the evidence boundary. `CursorIdentity` binds a token to
both an `EventSource` and a collector instance; `CursorToken` distinguishes
ordered sequence tokens (`seq-<epoch>-<position>`) from legacy numeric fixture
tokens and opaque values. Ordered tokens compare explicitly as equal, advancing,
or regressing. Opaque reordering is refused unless an explicit reset or wrap
control establishes a new epoch. A non-gap event that jumps an ordered range is
refused as an unmarked skip; a first-class `Gap` event is the only intentional
coverage discontinuity.

The journal keeps cursor epoch, status (`active`, `reset`, `wrapped`, or
`invalidated`), token kind, policy identity/version, the last event ID, and the
serialized replay boundary in the cursor table. Duplicate event IDs are
replayed as the original ingest sequence only when their complete semantic
envelope matches. A different event at the same source/collector/cursor, a
policy or replay-setting change without reset, a regression, an unknown
ordering, an invalidated source, or a skipped range fails closed before the
transaction can commit. `reset_cursor_with_boundary`,
`wrap_cursor_with_boundary`, and `invalidate_cursor` are durable control
operations; source replacement is a new collector identity, not an inferred
reset.

## Components

| Component | Responsibility | Current state |
| --- | --- | --- |
| CLI | Parse commands, print structured results, and surface refusal reasons | Fixture commands available; capture refuses |
| Fixture adapter | Read synthetic JSONL and validate the event contract | Available |
| Source adapters | Translate bounded platform observations into the envelope | Live adapters not shipped |
| Policy gate | Apply consent, selected scope, exclusions, private-context rules, and redaction | Required before live capture |
| Event envelope | Preserve source facts, provenance, evidence level, and schema version | Versioned contract is documented; journal ingestion requires an origin capability |
| Ingest writer | Bound memory, serialize writes, and commit event, cursor, policy reference, and diagnostics atomically | Bounded fixture writer is implemented and tested; live gate remains |
| Journal | Store local event metadata and encrypted payloads when the production key path exists | SQLite/WAL design documented; cursor contract migrations 0003–0004 and durable replay-boundary writes are implemented; Keychain production path not shipped |
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
5. The bounded writer persists the event, source cursor, policy reference, and
   diagnostics in one transaction when the live path is enabled. Queue pressure and
   unrecoverable source history become visible gaps.
6. Query, explanation, and export read committed records. They never mutate source
   history and never silently repair a gap.

## Storage boundary

The active journal is planned as one local SQLite database in WAL mode with one
writer and read-only readers. Its ordered migration catalog records each SQL
identifier, checksum, resulting schema version, tool version, and application time
before the journal is considered open. A missing, modified, reordered, future,
partially applied, or downgraded migration refuses startup; legacy v1 journals are
adopted only after their expected schema is verified. WAL improves reader/writer
concurrency, but it is not an encryption boundary and it does not make a source
complete. SQLite metadata,
temporary files, backups, and operating-system filesystem behavior remain part of
the threat model. The file-backed implementation applies the explicit policy in
[ADR 0003](adr/0003-sqlite-wal-active-journal.md): bounded busy waits and reader
snapshots, observable passive/truncate checkpoints, and refusal when remaining
frames or sidecar bytes exceed the configured limit. A database snapshot is made
only after a truncate checkpoint and never by copying a `-wal` or `-shm` file.

Production sensitive payloads require authenticated encryption with a macOS Keychain
backing key. That key path is not represented as shipped live-capture capability in
the current headstart. Missing keys or failed authentication must fail closed.

Payload bytes are now stored in a versioned `GRCE` envelope that records the cipher
algorithm, positive key generation, nonce, and authenticated ciphertext without ever
serializing key material. Readers retain a legacy nonce-plus-ciphertext compatibility
path while a migration is in progress. `KeyRotation` stages a new generation, resumes
from a key-free checkpoint, verifies every replacement, and retires the prior key only
at commit; a crash before commit therefore leaves the old ciphertext readable. Explicit
lost-key, compromise, and user-reset confirmations return bounded receipts that name
the destroyed generations and state when their ciphertext is unrecoverable. No cloud
recovery secret is introduced.

### Storage fault matrix

The fixture journal exposes an inert-by-default `FaultPlan` for recovery drills.
Named points bracket storage open/verification, migration SQL and commits, key
access, event/cursor/diagnostic writes, ingest commits, cursor controls, WAL
checkpoints, and database-only backups. A schedule can return a bounded
`InjectedFault` to prove transaction rollback or abort a child process to model
power loss. Each schedule has a bounded occurrence and seed; the minimized
regression fixture is `tests/fixtures/fault-schedules-v1.json`. The plan is an
explicit test capability and is never installed by the normal journal
constructors or live capture.

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
file but never replaces a symlink or hard link. WAL and SHM sidecars are checked
after every file-backed write; they are not independent backups. The backup helper
first runs a truncate checkpoint and copies only the database file, refusing a
sidecar destination.

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
