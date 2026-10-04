# Architecture

GHOSTRACE is a local, modular Rust application with a deliberately narrow data
path. The public source includes synthetic fixture tooling and user-invoked
macOS commands for selected-root FSEvents, shell wrapping, Git snapshots, and
explicit export/report. Ambient CLI capture remains disabled until the
remaining path-policy, recovery, and release gates pass.

GHOSTRACE is the event-observation and explanation layer in the portfolio. It does
not index source documents, perform OCR, or analyze TypeScript architecture. Those
are separate product boundaries; see [Product boundaries](BOUNDARIES.md).

## Pipeline

~~~text
source adapter
  ├─ fixture JSONL (current)
  ├─ explicitly consented selected-root FSEvents (API and CLI)
  └─ requested shell/Git context; opt-in frontmost context
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
the production Keychain design. These fixture commands are distinct from `live`
and `run`, whose explicit commands use login-Keychain custody and versioned policy.
`capture` remains an explicit refusal. GHOSTRACE has no network client; deliberately
wrapped programs are not network-sandboxed by `run`.

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
the contract for the optional Parquet cold archive. The archive does not replace the encrypted journal or JSONL export: it describes a derived,
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
encryption is not assumed. Automatic archive creation is forbidden, and deletion
semantics explicitly stop at the external-copy boundary.

The writer (`src/parquet_archive.rs`) is behind the opt-in `parquet` cargo feature,
built on the `parquet` crate without Arrow, so default and release builds do not
link it. It reads only a JSONL export that passes `validate_export`, never the
journal. Columns are flat, in profile order: `utf8` is `BYTE_ARRAY` (String),
`uint32` and `uint64` are unsigned `INT32`/`INT64`, and timestamps are `INT64`
nanoseconds since the epoch, adjusted to UTC. Nullable columns are `OPTIONAL`, the
rest `REQUIRED`. Row groups hold at most 16,384 rows or 64 MiB of row JSON.

The footer's key-value metadata, all under `ghostrace.archive.`, records
`profile_schema_id`, `profile_version`, `profile_sha256` (of the golden profile),
`source_manifest_sha256` (of the export's manifest line),
`source_event_body_sha256` (the export's event body digest), `row_count`,
`rows_sha256` (SHA-256 over each row's canonical JSON plus a newline, in order),
and the plaintext warning. The file is written to a `0600` temporary beside the
destination and synced. Every row is then read back, rebuilt into its JSONL event
record, and compared with the export after both pass through `EventEnvelope`, and
the footer counts and digests are recomputed. Only then is the file renamed into
place without replacing an existing file. Any failure removes the temporary.
`verify_parquet_archive` repeats the same comparison later.

### Explicit shell-wrapper metadata

[`schemas/shell-execution-metadata-v1.json`](../schemas/shell-execution-metadata-v1.json)
defines the only metadata the explicit user-invoked shell wrapper may submit. The
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
collector; the separate implemented wrapper remains explicit and consent-gated.

### Git repository and worktree identity

[`git-repository-worktree-identity-v1.json`](../schemas/git-repository-worktree-identity-v1.json)
defines the path-free identity boundary used by the explicit Git adapter. The
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

### Metadata-only Git snapshot boundary

[`GitSnapshotMetadata`](GIT_SNAPSHOT.md) is the pure-data privacy contract used by
the explicit Git snapshot adapter. It accepts an opaque repository identity, an explicit
SHA-1 or SHA-256 format, optional algorithm-tagged HEAD/tree/index IDs, bounded
status counts, branch and operation classes, and required source limitations.
It has no path, ref name, remote, message, author, filename, diff, patch, or
object-content field. Its constructors perform no filesystem, Git, network, or
object-database I/O; the separate adapter normalizes metadata before calling
them and must discard all other command output. Unknown source conditions are
represented explicitly rather than promoted to complete history. The snapshot
schema and digest are validated before `live git-snapshot` projects metadata and
history gaps into the journal. This explicit policy-gated command does not require
the persisted shell consent that `run` requires.

### Shell-wrapper lifecycle reference harness

[`fixtures/shell-wrapper-lifecycle-v1.json`](../fixtures/shell-wrapper-lifecycle-v1.json)
and `tests/shell_wrapper_lifecycle.rs` specify the lifecycle behavior an
explicit wrapper must preserve. The device-safe harness invokes `/bin/sh -c` with
cleared environment and null standard streams, then returns the native child exit
code or signal unchanged. It exercises normal and non-zero exits, shell built-ins,
pipelines, timeout, cancellation, and exec failure. Terminal closure and wrapper
crash are represented as explicit gaps with no completion, end time, exit code, or
success status. The reference harness is not a terminal collector: the separate
explicit `run` wrapper implements deliberately requested program execution, with
no PTY or ambient command/terminal collection.

### Explicit shell run wrapper

`ShellWrapper` (`src/shell_wrapper.rs`) is the only shell capture path. It is a
library adapter built like the selected-root collector: the caller renders a
consent preview for a policy document that enables the `shell` source, and the
wrapper refuses to run anything once consent is revoked. Each `run`/`run_in` call
executes exactly one user-supplied program with inherited standard streams and
environment, so the command behaves as it would in the terminal, but none of those
bytes are read, stored, or hashed.

A run commits `shell_started` before the child is spawned and a terminal event after
it exits, both under a live ingestion origin with the started event as parent:

- `shell_started` carries the wrapper session, `shell_kind: ghostrace-run`, the
  normalized executable basename token, and a working-directory class plus digest.
  A basename that is not a safe lowercase token is recorded as `unclassified`.
- The working directory is classified as `workspace_relative` (under a
  policy-selected workspace root), `home_relative`, `absolute_redacted`, or
  `unknown`. Its digest is domain-separated and anchored to the scope directory's
  device/inode; outside any scope it digests only the directory's own device/inode,
  so it cannot be matched against a dictionary of path strings.
- `shell_finished` carries the outcome class, exit code, duration, and the
  terminating signal for a signaled child. The wrapper returns the child's exit
  code, or `128 + signal`, unchanged to its caller.
- A program that cannot be started produces a `gap` (`shell_exec_failed`) with no
  end status and exit code 127.
- A wrapper killed before the child exits leaves an unmatched `shell_started`.
  Explanations report such a run as having no terminal observation, and
  `recover_incomplete_runs(older_than)` later closes each one with a
  `shell_run_incomplete` gap parented to the start, never with an end time or
  status. The age bound keeps a run that is still live in another process open.

While it waits, and only after the child has been spawned with default
dispositions, the wrapper ignores SIGINT and SIGQUIT (the terminal already delivers
them to the command) and forwards SIGTERM and SIGHUP to the child, so a terminal
close or a kill of the wrapper still ends in a recorded outcome. Runs are serialized
within one process because these dispositions are process-global. `executable_id`, `working_directory`, and
`signal` are optional v1 payload fields; envelopes without them are unchanged.

### Shell secret-leakage red-team boundary

[`fixtures/shell-secret-leakage-v1.json`](../fixtures/shell-secret-leakage-v1.json)
and `tests/shell_secret_leakage.rs` are a synthetic, unique-sentinel corpus for the
wrapper data boundary. The tests inject sentinels into arguments, environment,
standard input/output/error, executable names, working paths, failure messages,
prompt text, process titles, diagnostics, crash-report context, and command text.
Metadata validation, journal ingestion, diagnostics, exports, CLI output, and panic
output reject or omit every sentinel before GHOSTRACE retention. Process inspection
and operating-system crash reporting may expose synthetic process state outside the
application; those rows are documented as `os_visible_not_retained`, not claimed as
privacy guarantees. This red-team contract adds no event fields or executor of
its own; the separate consented `run` wrapper does not authorize ambient capture.

### Frontmost-application identity boundary

`src/frontmost.rs` is the normalization contract for the NSWorkspace adapter in
`src/frontmost_macos.rs`. `FrontmostRawObservation` is the only
input an adapter may pass: the transition (activated, deactivated, terminated),
time, bundle identifier, the developer-set `CFBundleName` and
`CFBundleShortVersionString` from the bundle's unlocalized Info.plist, whether the
executable is bundled, activation policy,
translocation flag, code-signing validity/ad-hoc/platform/team facts, and the
process ID and start time. It has no field for window titles, document names,
URLs, accessibility data, menu state, localized names, or screen content, and
strict deserialization rejects them without echoing their values.

`FrontmostNormalizer` keeps a lowercase bundle identifier (dropped if unsafe), the
bundle name (at most 64 printable characters with no path separators, control,
zero-width, or direction-override characters) and version (at most 32 characters of
ASCII letters, digits, and `. - _ + ( )`), each dropped whole rather than truncated
and never kept for an unbundled executable, a signing class (`developer` with a validated team ID, `platform`, `ad_hoc`,
`unsigned`, or `unknown`), a kind (`regular`, `helper`, `command_line`, `unknown`),
a location (`installed` or `translocated`), and a launch-instance digest salted per
journal in place of the process ID and start time. A bundle with no usable
identifier and no verifiable signature, or an invalid process, becomes an explicit
unknown app.

The adapter (`frontmost_macos`, behind the opt-in `frontmost` cargo feature so
default builds do not link AppKit) reads `NSWorkspace.frontmostApplication`, the
bundle's `infoDictionary`, `proc_pidinfo` for the start time, and
`SecCodeCopySigningInformation` for signing facts. It needs no Accessibility or
Screen Recording permission. The bundle path is read only to recognize App
Translocation and is not kept. `frontmostApplication` is updated through the main
run loop, so `FrontmostProbe::poll` must run on the main thread, which it pumps for
the polling interval before each read; observation times are the read time, within
one interval of the switch. An observed `com.apple.loginwindow` is a lock hint,
not proof that every lock/session or sleep/wake boundary was observed.

`ghostrace live apps` (`src/live/apps.rs`) runs the probe on the main thread under
its own `apps` policy (frontmost-app and lifecycle sources, root `frontmost`) with
a per-journal salt kept in the home's private configuration. Each activation is a
direct `frontmost_app_changed` event carrying the bundle ID, name, and version; the
session it ends is closed by inference at that moment with its dwell time. An
observed login window is treated as a screen-lock boundary; observing an app again
resumes coverage with a gap. Observer start/stop boundaries are also supplied by
the CLI. Password managers and user-excluded bundle IDs are never identified.

Each poll also feeds `FrontmostLifecycleMonitor` (`src/frontmost.rs`) a session
sample: wall time, uptime from `std::time::Instant` (which on macOS does not advance
during sleep), and `CGSessionCopyCurrentDictionary`'s console and screen-lock flags
(only those two keys are read). Wall time outrunning uptime by more than two seconds
is sleep, reported as `WillSleep` at the last awake sample, so a session's dwell
ends when sleep began; a locked screen, the login window, or another user on the
console suspends coverage until the session is active again, and the resumption is
a gap. Boundaries are accurate to the 250 ms poll, and a forward clock change is
also recorded as a gap. `tests/frontmost_lifecycle.rs` covers these transitions
with synthetic samples; locking and sleeping the reference device are manual
checks, because a test cannot lock the user's screen.

`FrontmostSessionTracker` suppresses repeated activations of the frontmost
instance, puts the dwell time on the event that ends a session, marks sessions
shorter than 500 ms as transient, gives no dwell to a deactivation it did not see
start, and clamps backwards clocks to zero. Activation is contextual evidence and
never establishes that an application caused a filesystem change. The corpus
[`fixtures/frontmost-identity-v1.json`](../fixtures/frontmost-identity-v1.json) and
schema [`schemas/frontmost-observation-v1.json`](../schemas/frontmost-observation-v1.json)
are exercised by `tests/frontmost_identity.rs`.

Coverage is bounded as well as identity. Every app record carries a `basis`:
`direct` when a notification reported it, or `inferred_closure` when the tracker
ended a session because the notification that should have ended it was not seen.
An activation while another session is open closes that session at the
activation time. When supplied to the tracker, sleep, screen lock, fast user
switching, and observer-stop events close the open session at the boundary and
emit a `suspended` coverage record; the
matching wake, unlock, session return, or observer start emits `resumed` with the
time coverage was lost. An observer start without a clean stop emits
`interrupted`, drops the open session without a dwell, and reports the interval
since the last observation as a gap, so no session is ever extended across
downtime. Private applications and user exclusions are replaced by an `excluded`
unknown app before any record exists. The transition table and sequences in
[`fixtures/frontmost-coverage-v1.json`](../fixtures/frontmost-coverage-v1.json)
cover startup, login, fast user switching, lock, sleep and wake, Mission Control,
termination, missed deactivations, and observer restarts; `tests/frontmost_coverage.rs`
also checks that no dwell interval contains a suspension or interruption.

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
`internal_storage_path` policy-blocked summary after the native stream stops or is
revoked. During observation only its bounded counter changes; emitting a summary
into the observed journal would itself be internal storage activity and could
invalidate an export's confirmed event snapshot. `internal_path_denials` remains
available in collector status. A crash can lose the pending aggregate; it is not
a completeness guarantee. Outside-scope summaries still persist during observation
([ADR 0006](adr/0006-internal-denial-summaries.md)).
Existing internal objects remain
denied after relocation through device/inode binding, while symlink redirects fail
closed during canonicalization and selected-root containment. Exact transport duplicates are
suppressed only when their source event ID, raw flags, and path digest all match a
bounded event-ID window; the suppression count is exposed in collector status and
never becomes a missing filesystem event. FSEvents does not promise ascending IDs
across coalesced deliveries, so each drained batch is ordered by event ID (except a
batch spanning an ID wraparound). A distinct delivery whose ID is still at or below
the committed cursor cannot advance it: it is counted as `out_of_order_events` and
recorded as an `fsevents_out_of_order` gap that recommends a rescan but, like
`cursor_jump`, leaves the cursor valid, so collection continues instead of stopping
with a cursor regression. Source coalescing, repeated modification,
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
Gap events are committed with their cursor advancement in the same transaction.
Durable restart/replay recovery is implemented and tested in the collector and
journal. Complete target-device sleep/wake, logout, detach, source-loss recovery
and release-scale lifecycle evidence remain gates; ambient CLI capture stays
disabled. Completed component tasks do not establish those broader release claims.

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
| CLI | Parse commands, print structured results, and surface refusal reasons | Fixture tooling plus explicit live/run commands; ambient capture refuses |
| Fixture adapter | Read synthetic JSONL and validate the event contract | Available |
| Source adapters | Translate bounded platform observations into the envelope | Explicit selected-root, shell and Git paths; optional frontmost path with incomplete lifecycle coverage; browser/Endpoint Security remain unimplemented |
| Policy gate | Apply consent, selected scope, exclusions, private-context rules, and redaction | Implemented and required before explicit live persistence; no ambient grant |
| Event envelope | Preserve source facts, provenance, evidence level, and schema version | Versioned contract is documented; journal ingestion requires an origin capability |
| Ingest writer | Bound memory, serialize writes, and commit event, cursor, policy reference, and diagnostics atomically | Implemented for fixture and explicit live paths; large-journal authenticated-write cost remains open (#403) |
| Journal | Store local event metadata and encrypted payloads | SQLite/WAL, migrations, replay boundaries and authenticated state implemented; explicit live path uses login Keychain, not a signed production data-protection release |
| Explain/export | Produce deterministic evidence-linked explanations and explicit exports | Fixture and explicit live surfaces, preview-bound JSONL, optional Parquet and offline HTML |
| Query | Return bounded, policy-scoped pages from a stable logical ingest snapshot | Encrypted-token pagination is implemented and covered by concurrent-ingest, deletion, token-negative, and migration tests |

The live CLI's `status` and `timeline` currently call `Journal::events`, which
materializes and decrypts all events before counting or limiting displayed rows.
Their output limit is not a read-memory bound; release-scale read performance
remains unverified. This path is distinct from the paginated query API above.

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

Each active journal is one local SQLite database in WAL mode, with serialized
write transactions and snapshot readers. A central service owning all CLI/UI
writes remains future work; SQLite coordinates separate explicit CLI processes.
Its ordered migration catalog records each SQL
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

Explicit live payloads use authenticated encryption with a login-Keychain backing
key. The data-protection provider remains the default library custody choice,
but signed/entitled production distribution is not complete. Missing keys or
failed authentication fail closed; login custody is not an implicit fallback.

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

### Authenticated journal state (tasks 0088 and 0175)

Mutable metadata that SQLite does not authenticate has a separate versioned
keyed anchor. Canonical bytes are length-delimited under the domain separator
`ghostrace:authenticated-journal-state:v1`; component digests bind event order
and identity set, event metadata/ciphertext, cursors, policy history, and
diagnostics. The head MAC also binds the chain epoch, chain-start boundary, key
generation, and deletion digest. The anchor contains no key material, paths,
plaintext, or retained event identifiers.

The first v2 write on a legacy v1 journal is a distinct migration boundary. It
verifies the complete v1 canonical snapshot and head MAC, then builds the v2
commitment maps and operation-ledger root while holding one `IMMEDIATE`
transaction. That one-time promotion/startup cost is not the 100,000-event hot-
write bound and must be measured separately.

After promotion, each event/cursor/policy/diagnostic write either reuses a
post-commit cache or performs its complete read-side integrity and
authentication preflight before `BEGIN IMMEDIATE`; it then re-reads the
connection's `data_version` and full anchor identity under the retained guard.
A changed version or identity rolls back and retries from a new outside-lock
snapshot, including on the cached fast path. Once admitted, the write consumes
only pending trigger rows, updates the affected commitment terms, appends one
operation-ledger entry, and commits; it does not scan historical events under
the write lock. Dropping the guard on a refusal rolls back the whole operation.

Each write has one short-lived key scope shared by preflight, authentication,
payload encryption, and anchor advancement, including preflight retries. It pins
the active generation and resolves each required generation from the backing
provider at most once, without retaining keys between writes. Ordinary writes
use one generation and make exactly one provider read, for both file-backed and
in-memory journals. In-memory writes keep the anchor check under the same guard;
sharing the key preserves their single-read contract. Rotation or preflight over
retained ciphertext from older generations needs one read per distinct generation.

`authenticated-check` performs the full v2 recomputation. The explicit
100,000-event device lane in `tests/authenticated_state.rs` reports seed/setup
timing independently, reopens and warms a connection, bounds steady-state writes
using `GHOSTRACE_AUTH_HOT_WRITE_BOUND_MS`, and then exercises two fresh
connections and two separate processes with the unchanged 250 ms default busy
timeout. The opt-in legacy
migration lane measures the separate v1-to-v2 promotion boundary. Component
maps and operation-ledger verification remain full-check paths and are not
substituted by the hot-write measurement.

The reference measurement on 2026-10-04 used an Apple M1 MacBook Pro with
8 GB RAM, macOS 26.6.2 (25G83), arm64, Rust/Cargo 1.88.0, and the unoptimized
all-features test profile. With 100,000 synthetic events, 64 steady-state
single-event writes had median 3.696 ms, p95 6.152 ms, and maximum 11.364 ms.
The predeclared bound is 200 ms. Whole-write wall time conservatively bounds
the `IMMEDIATE` lock interval; this lane does not directly instrument the lock.
Each of two connections and two separate processes completed 32 writes with
no timeout at the default 250 ms. Seeding took 110.320 s, the first write after
reopening (including full preflight) took 14.486 s, and the final full check took
9.286 s. Startup and each of the 64 hot writes made exactly one backing-provider
key read. Startup/external-commit scans, legacy promotion, rotation, retention,
and multi-event batches are outside the steady-state single-event bound.
The preceding measurement's 28.682 s startup was close to the default 30 s reader
limit. That limit and the full-scan algorithm remain unchanged; neither startup
result bounds other devices or larger operation histories.

Reproduce the device lane under a 600 s process-group watchdog:

```sh
GHOSTRACE_AUTH_HOT_WRITE_BOUND_MS=200 cargo test --locked --all-features \
  --test authenticated_state \
  one_hundred_thousand_events_and_two_default_timeout_writers \
  -- --ignored --exact --nocapture
```

Setup, each process result, the full sample vector, and the final authentication
result are emitted separately.

A confirmed retention delete advances the chain epoch and records only
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

The default macOS library provider uses the data-protection Keychain generic-password path:
non-synchronizable items, `WhenUnlockedThisDeviceOnly` access control, and an explicit
service/account identity. The default app has no access-group entitlement; a signed
helper may use one only when its bundle entitlement matches. Login-session availability
and the data-protection requirement are checked before returning key bytes, so an
unsigned CLI, locked session, duplicate item, or malformed item fails closed without
falling back to the legacy file keychain.
This describes data-protection custody, not the only macOS provider mode. The
explicit unsigned live CLI deliberately selects login-Keychain custody at home
initialization; it never silently switches modes after a data-protection failure.
Signed/entitled production distribution remains a separate release gate.

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

FSEvents event IDs are global across the whole system, while a stream only
delivers events for its selected roots, so consecutive deliveries are normally
non-contiguous. The collector therefore records native events with a `sparse`
cursor (`sparse-0-<event ID>`): positions must strictly increase, but a hole is
not a loss. The journal's skipped-position check applies only to contiguous
`sequence` cursors. Loss on a live stream comes solely from the source's own
drop and rescan flags, and across a restart the replayed history covers the
interval, with history unavailability reported by the existing startup gaps.
Journals that committed the earlier `cursor-<id>` form continue forward.

## Native-messaging protocol

`src/native_messaging.rs` is the strict codec for the extension-to-host channel
in [ADR 0005](adr/0005-browser-transport-and-permissions.md). The executable
bridge is exposed as `ghostrace native-host run`, but no browser extension or
release manifest is claimed by this slice. `FrameDecoder` reads Chromium's 4-byte
native-endian length prefix incrementally and refuses a zero, oversized (above
64 KiB), or truncated frame before allocating its body. `parse_message` rejects
invalid UTF-8, then scans structure (at most 8 levels of nesting and 256 JSON
values, ignoring brackets inside strings) before strict typed deserialization of
the four v1 message types (`hello`, `navigation`, `heartbeat`, `goodbye`); unknown
types, fields, and transition values are refused.

`ProtocolSession` requires `hello` first with exactly protocol version 1 and
sequence 1, ends the session on any second `hello` (renegotiation or downgrade),
rejects a repeated or backwards sequence number as a replay, reports skipped
numbers as `AcceptedAfterGap` so the caller records a gap, times out after 120
seconds of silence, limits a session to 200 messages per 10-second window, and
refuses anything after `goodbye`. Errors are fixed values. `tests/native_messaging.rs`
covers every refusal, arbitrary chunk boundaries, and 20,000 deterministic fuzzed
frames with no panic and no echoed content.

## Local service socket

`LocalService` (`src/local_service.rs`) is the only way a local client will reach
the service. Its `browser_navigation_v1` method is the sole browser-ingestion
entry point in this slice: the request contains an event ID, approved browser
label, `CanonicalNavigation`, observation time, and a bounded missing count. It
has no raw URL, query, fragment, userinfo, or path. A mandatory relay proof
wraps the admission; the service checks the active pairing and its MAC before
using the writer. The service rejects a
path-bearing `CanonicalNavigation` rather than silently dropping the field. It
binds `ghostrace.sock` in a
directory that must be a real directory owned by the current user with no group
or other access (created with mode 0700 if absent, never followed through a
symbolic link), sets the socket to mode 0600, and replaces only a stale socket it
owns; any other file at that path is refused and kept. No TCP, UDP, or HTTP
listener exists. The native host receives the current service socket and
per-start instance UUID explicitly; it never opens the journal or receives its
key. `BrowserIngestService` is the service-side single-writer adapter that
projects the accepted admission through the existing `Writer`.

Each connection is admitted in order: the peer must be the same effective user
(`getpeereid` on macOS, `SO_PEERCRED` on Linux); the length-prefixed request must
be at most 64 KiB of strict JSON; the protocol version must be 1; the request must
name the service's per-start instance ID, so a client cannot talk to a different
or restarted service by accident; the deadline must be between 1 ms and 30 s; the
request ID must not have been seen in the replay window; and the requested
capability (`ingest`, `read`, `export`, `policy`, `lifecycle`, `admin`) must have been
granted when the service was bound. Nothing is granted by default, and refusals
are fixed error values. `tests/local_service.rs` covers each check.

## Native-host manifest registration

`NativeHostInstaller` (`src/native_host_manifest.rs`) registers the GHOSTRACE
native host for Chrome, Chrome Beta, Chromium, or Edge by writing
`com.alisinadevelo.ghostrace.json` into that browser's per-user
`NativeMessagingHosts` directory. The manifest is fixed: name, description, the
absolute host binary path, `stdio`, and exactly one allowed origin,
`chrome-extension://<32 letters a-p>/`; wildcards and malformed IDs are refused
before anything is written. `plan` reports the exact file and action; `install`
exclusive-creates or atomically replaces only a manifest whose digest matches the
receipt beside it, so moving the host binary is an upgrade; `verify` reports
intact, missing, drifted, or not installed; `uninstall` removes the manifest only
while it is byte-identical to what was installed. Removing the manifest is how a
browser stops launching the host, so there is no separate disable state. A
manifest GHOSTRACE did not write, a symlinked or group/other-writable directory
anywhere below the support root, a linked or foreign-owned manifest, and a
manifest edited to admit another origin are all refused, and other hosts'
manifests are never touched. `tests/native_host_manifest.rs` covers each case.

## Browser pairing

`src/browser_pairing.rs` implements the pairing and message-authentication layer
of [ADR 0005](adr/0005-browser-transport-and-permissions.md). A `PairingRequest`
carries everything the user approves: browser channel, profile class, extension
ID, SHA-256 of the extension's public key, SHA-256 of its reviewed permission set,
event classes, retained fields, and the private-context policy. Approval creates a
`PairingRecord` with a random 32-byte secret (handed to the extension once and
redacted from `Debug`), a 90-day expiry, and a revocation flag.

On connect the extension sends a `ClientHello` with its pairing ID, identity
digests, and a fresh client nonce. A different extension ID or unknown pairing is
refused as not paired; a changed key digest (replaced or sideloaded extension), a
changed permission digest, or an expired approval requires re-pairing; a revoked
pairing is refused. An admitted session derives its key as HMAC-SHA256 of the
secret over a domain tag and both nonces, with a fresh host nonce per session, and
every message carries HMAC-SHA256 over its sequence number and frame body,
verified in constant time. A transcript from an earlier session, including one
replayed after a host restart, therefore never verifies; ordering and replay
within a session are enforced by `ProtocolSession`. HMAC is built on the crate's
existing SHA-256 and checked against RFC 4231 vectors in `tests/browser_pairing.rs`.

## Native host session

`NativeHostSession` (`src/native_host.rs`) composes the browser pieces for one
native-messaging connection. `hello` carries the pairing ID, extension ID, key and
permission digests, and a 32-byte client nonce in hex; the session admits it
against the stored `PairingRecord`, derives the session key with a fresh host
nonce, and replies `welcome` with that nonce (or ends with a fixed `refused`
code: `not_paired`, `revoked`, `re_pairing_required`, `unauthenticated`, or
`protocol_error`). Every later message carries `mac`, HMAC-SHA256 under the
session key over the message's sequence number and its canonical input:

```text
ghostrace-nm-v1\nnavigation\n<seq>\n<0|1 private_context>\n<transition>\n<url>
ghostrace-nm-v1\nheartbeat\n<seq>
ghostrace-nm-v1\ngoodbye\n<seq>
```

The browser serializes messages itself, so the MAC is defined over these typed
fields rather than JSON text, and the extension builds the identical bytes. The
MAC is verified before sequence handling, so an unauthenticated message cannot
advance or disturb session state; then `ProtocolSession` applies its ordering,
replay, deadline, and rate rules; then a navigation is reduced by
`CanonicalNavigation` or counted as refused. The caller closes the connection on
any error. `tests/native_host_session.rs` drives the full handshake from the
extension's side.

## Native host bridge and pairing storage

`NativeBridge` (`src/native_bridge.rs`) is the executable per-connection
boundary. It reads the bounded encrypted pairing store, selects the requested
pairing record, re-checks revocation and
approval expiry on every post-handshake frame, keeps `UrlShapePolicy::OriginOnly`, and forwards a
typed admission to `LocalServiceClient`. The pairing CLI (`pair`, `list`, and
`revoke`) is explicit and user-consented; pairing records are bounded,
authenticated, and atomically persisted in `pairings.enc` beside an independent
0600 `pairing.key`. The one-time extension secret is printed only by the approval
receipt and is absent from list output; the host retains its copy inside the
authenticated ciphertext so a restart can authenticate the paired extension.
The host and service each hold a shared pairing-store lease during admission.
The service hands an owned lease to the writer queue; the worker retains it
through transaction completion and receipt delivery, including after a host or
service acknowledgement timeout. Revocation needs an exclusive lease, so a successful
revocation receipt cannot race an in-flight admission; lock contention returns
the fixed `pairing_busy` refusal after at most five seconds. Atomic publications
sync the file and parent directory before acknowledgement.

The production stdio loop polls the native-messaging file descriptor before each
read, so the pre-hello and partial-frame cases have a real idle deadline rather
than relying on a blocking `Read`; the protocol's connection-start anchor means
rejected pre-hello frames cannot extend that deadline. Native stdout writes are
also nonblocking and deadline-bounded, so a browser that stops reading cannot
hold the process forever. A clean `goodbye` closes the protocol; any complete or
partial bytes after it are refused as trailing/framing data. Chrome
launches the absolute manifest binary with exactly one `chrome-extension://id/`
argument. `main` validates that argument before opening the pairing store or
consuming stdin, and the authenticated `hello.extension_id` must match it. The
service publishes only a bounded, private endpoint receipt containing its socket
path and fresh instance UUID; it never publishes a journal path or key.

The retained-fields approval is intentionally constrained to `origin` because
the existing browser event payload has no field for `CanonicalNavigation`'s
optional path class. A path-retaining request is refused during pairing, and a
path-bearing service admission is refused at the service boundary. This is a
deliberate visible policy boundary, not silent field loss. Synthetic tests cover
restart, revocation, key/permission drift, replayed transcripts, private-context
refusal, framed stdio, service capability denial, and the journal projection;
they do not claim a browser installation or release integration.

Same-UID socket transport is not browser authentication. The service verifies a
separate domain-separated HMAC with the approved pairing secret, binding the
service instance, request ID, pairing ID, client nonce, sequence, stable event
ID, observation timestamp, browser label, canonical navigation, and gap count.
A copied proof under a new request or service instance is refused. Browser
channel, profile, and extension key/permission digests are declared credential
scope; they are not OS-attested browser identity. The host requires at least one
intact installed manifest matching its binary and exact caller extension ID,
and refuses any detected manifest drift before consuming browser input.

The delivery ID derives from the pairing, client nonce, and sequence. A retry
with the same delivery contract returns the original durable sequences and
timestamps; a changed canonical payload, gap count, or policy is refused. The
extension must retain its delivery nonce and sequence for retries and use a new
nonce for a new delivery stream. Timeout/transport failures return the fixed
`journal_uncertain` code because a transaction may already have committed.
Heartbeat, refused-navigation, and goodbye gaps have no origin payload; they
return `journal_error` rather than claim a persisted gap. Private-network
navigation is refused by the journal projection because the legacy payload
cannot represent a withheld host without inventing an origin.

This slice provides the host CLI and service handler API. A long-running service
owner must construct `BrowserIngestService::new_with_pairing_store`, explicitly
grant `Ingest`, and publish its endpoint. Extension packaging, a real browser
launch, user consent UI, service lifecycle wiring, and release integration are
separate tasks.
