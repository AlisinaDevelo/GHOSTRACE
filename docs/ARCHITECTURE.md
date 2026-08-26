# Architecture

GHOSTRACE is a local, modular Rust application with a deliberately narrow data
path. The public slice replays synthetic JSONL fixtures and now contains an explicitly
enabled selected-root FSEvents source. Ambient CLI capture remains disabled until the
remaining path-policy, recovery, and release gates pass.

GHOSTRACE is the event-observation and explanation layer in the portfolio. It does
not index source documents, perform OCR, or analyze TypeScript architecture. Those
are separate product boundaries; see [Product boundaries](BOUNDARIES.md).

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
        ├─ policy-bounded correlation rules
        ├─ explicit JSONL export
        └─ signed checkpoint → verified-copy repair → explicit gap manifest
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
        +--> preview --journal <path> --output <JSONL>
                    |
                    +--> export --journal <path> --output <JSONL> \
                         --confirm-plan <digest> --confirm-snapshot <digest>
```

`init` is idempotent and runs the same hardened SQLite path checks as the library.
`ingest` reopens the durable journal in a separate process, validates the fixture
origin and deny-by-default policy, and commits the batch before reporting success.
`explain` and `preview` reopen that journal with the same synthetic fixture key and
therefore exercise the persistence boundary rather than an in-memory shortcut.
`export` recomputes the plan and journal snapshot and refuses to write unless both
digests match the explicit confirmation from the preview. The receipt records only
the destination class and artifact digests, never the destination path.
The key is intentionally deterministic only for the synthetic headstart; it is not
the production Keychain design. `capture` remains an explicit refusal, and no CLI
command enables a live collector or network path.

## Checkpoint and repair boundary

The checkpoint command performs a bounded SQLite integrity/foreign-key check,
verifies the local authenticated anchor, checkpoints the WAL, and signs a
path-free receipt with the configured local key. The receipt binds database
bytes, journal schema, policy-table digest, chain epoch/head, event count and
maximum sequence, key generation, the integrity-report digest, and RFC3339
verification time. It is a local-integrity receipt, not remote attestation or a
legal chain-of-custody claim.

The repair command never rewrites its source. It requires a clean checkpoint,
copies only the checkpointed database file, re-verifies the copy, and then
removes only bounded ingest-sequence intervals that have no child or cursor-tail
references. Each removed interval is replaced by a repair-origin gap event in
one transaction. A path-free manifest records before/after identities and
integrity digests, dropped and reconstructed counts, interval bounds, and gap
count. The normal writer refuses to ingest when the SQLite data version
indicates an external change until a fresh integrity check succeeds. The
recovery-demo command exercises this workflow on synthetic unreferenced events.

## Export schema and manifest boundary

JSONL export is a two-part contract: one strict `manifest` record followed by the
body records. [`schemas/export-registry-v1.json`](../schemas/export-registry-v1.json)
registers stable IDs and version `1` for manifest, event, gap, claim, policy, and
source-coverage records. Each descriptor declares `strict` compatibility, rejects
unknown fields, and points to a checked-in golden example. The manifest binds the
registry and tool versions, schema-version map, deterministic `all_committed`
query scope, policy profile identities, coverage gaps, and body-only record counts,
byte lengths, and SHA-256 digests. Body-only accounting deliberately excludes the
manifest line from its own digest so the contract is not self-referential.

`validate_export` is the consumer gate. It parses the manifest first, verifies its
registry and scope, then accepts only declared event records with the event schema
version and the shared `(observed_at, ingest_seq, event_id)` stable order. It
compares the declared body count, byte length, and digest before returning a
validated result. Unknown fields, mixed versions, undeclared record types,
duplicate/order regressions, or any accounting drift are bounded errors; no
caller can treat a partially validated body as a complete export.

### Derived Parquet archive profile

[`schemas/parquet-archive-profile-v1.json`](../schemas/parquet-archive-profile-v1.json)
and its [golden profile](../fixtures/parquet-archive-profile-v1.golden.json) define
the contract for a future optional Parquet cold archive. The profile is not a writer
and does not replace the encrypted journal or JSONL export: it describes a derived,
explicit plaintext boundary that must be validated before publication. Version `1`
has exactly 23 columns. Event identity, both timestamps, source/kind, provenance,
policy identity, evidence, causal parent, and canonical payload JSON are retained
without lossy coercion. Gap payload facts have dedicated nullable columns; the
essential gap source, reason, and dropped count are required on a gap row and every
gap column is null on other event kinds.

Rows sort by `(observed_at, ingest_seq, event_id)`, matching the query and JSONL
contracts. Provenance and policy mappings are exact and unknown values reject.
Evolution is additive-nullable only: additions, removals, and type changes require a
new profile version, while undeclared columns are rejected. Streaming validation is
bounded to 23 columns, 1 MiB per row, 10 million rows, and 64 KiB of profile metadata.
The profile requires Zstandard compression, disables dictionary encoding, column
statistics, and page indexes to reduce metadata leakage, and records that Parquet
encryption is not assumed. A future writer must use mode `0600` temporary files,
atomic publication, cleanup on failure, and leave the source journal untouched.
Automatic archive creation is forbidden, and deletion semantics explicitly stop at
the external-copy boundary.

### Explicit shell-wrapper metadata

[`schemas/shell-execution-metadata-v1.json`](../schemas/shell-execution-metadata-v1.json)
defines the only metadata a future user-invoked shell wrapper may submit. The
strict v1 record contains an opaque wrapper session, a normalized executable
basename identity, a working-directory class plus root-scoped digest, start and
end timestamps, an outcome class, an exit code, and a signal. The raw working
directory is never a field; the executable identity cannot contain a path,
credentials, or shell text. Outcome validation requires `0` for success, a
non-zero exit code for failure, a signal with no exit code for signaled
termination, and no status details for unknown outcomes. End time cannot precede
start time and a wrapper run is bounded to seven days.

The schema has no representation for arguments, environment variables, standard
input/output, shell history, aliases, command text, or expanded command text.
`ShellExecutionMetadata` uses deny-unknown-fields deserialization plus semantic
validation, and its field registry records the semantic and sensitivity class of
every retained field. This is a data contract, not a shell executor or ambient
collector; a later wrapper must remain explicit and policy-gated.

### Git repository and worktree identity

[`git-repository-worktree-identity-v1.json`](../schemas/git-repository-worktree-identity-v1.json)
defines the path-free identity boundary for the future explicit Git adapter. The
adapter resolves Git's common object database and worktree metadata, then passes
only device/file identity values to `GitIdentity::from_stable_parts` (or
`from_paths`, which reads and immediately discards directory metadata). The
serializable result contains a domain-separated SHA-256 digest for the object
database, an optional worktree digest, the caller-owned opaque selected-root ID,
an explicit source scope, and a repository-kind enum. Remote URLs, credential
helpers, config values, reflog messages, and raw paths are not accepted fields.

`GitIdentity::continuity_from` compares repository identity first, then worktree
identity and source scope. A moved directory is continuous when the selected-root
binding is retained; a clone or repository reinitialization is
`repository_changed`; `git worktree add` is `worktree_changed`; and a changed
selected-root/source binding is `scope_changed`. Bare repositories explicitly
omit a worktree digest, while submodules use a distinct source scope and kind.
These are identity and continuity semantics only: the adapter must supply stable
filesystem metadata and must not turn a path, remote, reflog, or Git command
output into retained evidence.

### Shell-wrapper lifecycle reference harness

[`fixtures/shell-wrapper-lifecycle-v1.json`](../fixtures/shell-wrapper-lifecycle-v1.json)
and `tests/shell_wrapper_lifecycle.rs` specify the lifecycle behavior a future
explicit wrapper must preserve. The device-safe harness invokes `/bin/sh -c` with
cleared environment and null standard streams, then returns the native child exit
code or signal unchanged. It exercises normal and non-zero exits, shell built-ins,
pipelines, timeout, cancellation, and exec failure. Terminal closure and wrapper
crash are represented as explicit gaps with no completion, end time, exit code, or
success status. This is a test contract only: GHOSTRACE does not ship a shell
executor, PTY, terminal collector, or command capture path.

### Shell secret-leakage red-team boundary

[`fixtures/shell-secret-leakage-v1.json`](../fixtures/shell-secret-leakage-v1.json)
and `tests/shell_secret_leakage.rs` are a synthetic, unique-sentinel corpus for the
future wrapper boundary. The tests inject sentinels into arguments, environment,
standard input/output/error, executable names, working paths, failure messages,
prompt text, process titles, diagnostics, crash-report context, and command text.
Metadata validation, journal ingestion, diagnostics, exports, CLI output, and panic
output reject or omit every sentinel before GHOSTRACE retention. Process inspection
and operating-system crash reporting may expose synthetic process state outside the
application; those rows are documented as `os_visible_not_retained`, not claimed as
privacy guarantees. This red-team contract adds no event fields and does not ship a
shell executor or ambient capture path.

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

## Snapshot query contract

`Journal::query_page` executes inside the existing bounded read-snapshot
transaction. The first page captures the current maximum ingest sequence as an
upper bound; later pages use that bound from an authenticated token, so events
ingested after page one cannot appear mid-result. Ordering contract version `1`
uses known source `observed_at`, then durable `ingest_seq`, then canonical
`event_id`. Export uses the same key. Equal timestamps therefore have a
deterministic tie-breaker without treating display order as causality; an absent
source timestamp is explicit adapter input and falls back to ingest sequence.

Source observation time, local `ingested_at`, and optional process-local
monotonic sequence are separate timing facts. Clock rollback, leap-boundary
adjustments, sleep-sized gaps, delayed batches, equal timestamps, and missing
source time are reported as temporal ambiguity in analysis and explanations.

`QueryRequest` binds the policy profile ID/version and its scope digest, optional
source/root/kind/time filters, and page size. Root filtering decrypts candidate
rows from the bounded snapshot because payloads are encrypted at rest; the
stable cursor still advances over the full ordered stream, so non-matching roots
cannot cause duplicates or skips. Policy-blocked summaries are never returned as query events;
they remain visible through coverage statuses. The page token is an encrypted
local capability, not a caller-editable offset: it also carries the query digest,
event/storage schema versions, ordering-contract version, issue/expiry time,
snapshot upper bound, and last ordering key. Forged, expired, cross-profile,
changed-filter, and schema-changed
tokens fail with bounded refusal classes without echoing token contents. A
retention or deletion operation may remove a row after the snapshot; pagination
does not resurrect it or fabricate a continuity claim. New writes remain outside
the original logical snapshot.

Every page also carries coverage contract version `1`. Coverage scans use the
same policy, source, time window, and snapshot boundary but deliberately ignore
the requested event-kind filter, so a filesystem query cannot hide a relevant
gap, denial marker, or collector stop. Gap intervals are conservative
open-ended intervals when the source supplies no end boundary. Statuses
distinguish observed events, no events observed, source disabled, policy denied,
source gap, retention deletion detected between pages, and unknown history.
Callers may set `include_coverage=false`, but the response then sets
`coverage.opted_out=true` rather than silently presenting an incomplete result.
The query token contract is version `3` because the authenticated snapshot now
binds root-scoped requests and excludes policy-blocked summaries from event
pages, in addition to the matching-row count used to detect retention deletion.

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
| Query | Return bounded, policy-scoped pages from a stable logical ingest snapshot | Encrypted-token pagination is implemented and covered by concurrent-ingest, deletion, token-negative, and migration tests |

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

### Retention deletion and integrity boundary

The retention planner is read-only until a caller supplies its exact plan
digest, candidate-set digest, and ingest snapshot boundary to `retention-delete`.
That command acquires an immediate SQLite transaction, rechecks every candidate,
rejects scope drift, cursor-tail references, and unselected child events, then
deletes selected rows in reverse ingest order. Its receipt is explicitly
logical-only: it does not run `VACUUM`, destroy key material, or remove external
copies. A failure before commit leaves all rows intact.

`integrity-check` runs SQLite integrity and foreign-key checks on a bounded,
path-free read snapshot. It provides recovery guidance but never repairs the
database. A failure requires preserving the original and performing any repair
on a private verified copy with before-and-after receipts.

### Authenticated journal state (task 0088)

Mutable metadata that SQLite does not authenticate has a separate versioned
keyed anchor. Canonical bytes are length-delimited under the domain separator
`ghostrace:authenticated-journal-state:v1`; component digests bind event order
and identity set, event metadata/ciphertext, cursors, policy history, and
diagnostics. The head MAC also binds the chain epoch, chain-start boundary, key
generation, and deletion digest. The anchor contains no key material, paths,
plaintext, or retained event identifiers.

Every event/cursor/policy/diagnostic transaction refreshes the anchor before
commit. A confirmed retention delete advances the chain epoch and records only
the plan/candidate digests, snapshot boundary, and counts. After bootstrap, a
missing anchor is a failure and is never silently reseeded. `authenticated-check`
reports bounded insertion, deletion, reorder, edit, replay, truncation, cursor
rollback, policy substitution, diagnostic tampering, anchor, and key anomalies.
Replay is a verifier-only field match over event fields excluding ingest
sequence and event identity; it makes a copied row observable without claiming
that two legitimate identical observations are impossible. A valid report
authenticates only the configured local key; it is not origin attestation or a
legal chain-of-custody claim.

Key rotation is a chain boundary. A provider retains the previous generation
while an existing journal is opened and its old state is verified; the next
authenticated write re-anchors at the boundary, increments `chain_epoch`, and
binds the new generation into `chain_start_mac`. Older generations are retired
only after every state and ciphertext that names them has been independently
verified.

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

Claim grammar version `1` is the only renderer for explanation statements. Each
event kind selects a template descriptor declaring required facts, the rule for
preserving the event's evidence label, prohibited implications, and gap behavior.
The renderer supports only bounded `en` and `en-GB` locales, includes the cited
event ID in structured and textual output, and refuses template text containing
intent, completeness, process-attribution, unsupported causality, or old-to-new
rename implications. A gap either appears as an explicit status or limits the
interpretation of an otherwise observed fact.

### Cross-source correlation boundary

The correlation registry is version `1` and currently contains one rule:
`cross_source_temporal_adjacency` (rule version `1`). Its descriptor is the
source of truth for permitted inputs, exclusions, output evidence, bounds, and
counterexample classes. `CorrelationQuery` carries the policy profile identity,
scope digest, selected source set, bounded time window, and maximum input count.
The evaluator calls the policy gate before reading event metadata and never
reads selected-root strings or payload fields outside that authorization
boundary. It emits `inferred` only for two distinct, authorized, direct or
contextual observations within 60 seconds. Gaps, policy denials, unknown
evidence, equal timestamps, and source clock rollback are explicit `unknown`
results. Registry and rule versions participate in explanation identity and
are recorded in export manifests.

### Explanation determinism and counterexamples (task 0082)

`fixtures/explanation-counterexamples-v1.json` is the manifest-bound contract for
the explanation renderer's deterministic and fail-closed behavior. The golden
matrix names all twelve claim templates, all four evidence levels, both gap
states, and explicit unknown outcomes for coverage gaps, policy denials, source
errors, and unknown evidence. `tests/explanation_determinism.rs` compares
serialized claims across repeated rendering, ingestion permutations, equal source
timestamps, irrelevant events, and query page boundaries. Its mutation cases
remove a required cross-source observation or a parent observation and require an
unknown correlation or a shorter explanation chain. The corpus is synthetic,
offline, and contains no user data.

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
