# Privacy model

GHOSTRACE exists to help a person inspect a bounded sequence of local changes. It is
not designed to reconstruct everything a person did. The current public headstart is
local and offline: a selected-root collector API exists only behind explicit consent,
while the ambient CLI remains disabled.

## Defaults

- Collection is deny-by-default and requires explicit, versioned consent when live
  sources are introduced.
- Policy documents are strict and versioned. Unknown versions, duplicate identities,
  downgrade attempts, and semantic changes without explicit reconfirmation fail
  closed before a candidate observation can be retained.
- The baseline has no network client, telemetry, cloud sync, URL fetching, or silent
  upload path.
- Private browsing and private application contexts are excluded by default.
- The baseline does not use keylogging, microphone input, screen recording,
  clipboard data, window titles, page contents, or file contents.
- Root, Full Disk Access, Accessibility, and Automation permissions are not required.
- A rejected value is not retained merely to explain why it was rejected.
- Gaps, denials, and uncertain attribution are visible instead of being filled in.

These are product boundaries, not suggestions for a future configuration screen.

## Data inventory

| Data class | Purpose | Default state | Retention rule |
| --- | --- | --- | --- |
| Synthetic fixture event | Exercise parsing, explanation, and export | Allowed in developer headstart | Checked-in fixtures must contain no user data |
| Event ID, schema version, source, kind, timestamps, and policy ID/version | Identify, order, and audit the policy that accepted an observation | Fixture-only now; live only after policy | Bounded by the journal policy |
| Source cursor and status | Describe coverage and restart state | Not ambiently collected now | Persist with the event when live capture ships |
| Selected path metadata | Describe a permitted filesystem change without reading content | Explicit selected-root API only | Canonicalized, policy-checked, hashed, and bounded before persistence |
| Policy decision and reason | Explain why an observation was accepted, denied, or redacted | Required for live design | No blocked sensitive value is retained |
| Evidence level and gap | Express what the source supports and what it cannot | Part of the event contract | First-class records |
| Payload | Carry the minimum normalized source facts | Fixture-only plaintext may be shown by an explicit command | Production payloads require Keychain-backed authenticated encryption |
| Export | Give the user a requested portable view | Explicit command only | Written to the destination chosen by the user |

The exact fields are versioned in [EVENT_MODEL.md](EVENT_MODEL.md). A field is not
privacy-safe merely because it is called metadata; paths, timestamps, identifiers,
and source flags can be sensitive.

The macOS provider stores only the journal wrapping key as a generic password in the
data-protection Keychain. It sets `kSecUseDataProtectionKeychain`, requires
`kSecAttrSynchronizable=false`, and uses `WhenUnlockedThisDeviceOnly` access control.
The default service/account are `com.alisinadevelo.ghostrace.journal` and
`journal-wrapping-key-v1`; an access group is optional and must match the signed bundle
entitlement. An unsigned command-line helper or a locked login session has no fallback:
missing, inaccessible, duplicated, or malformed items produce redacted refusal errors.

### Key lifecycle and recovery

Encrypted payloads carry only a version, algorithm, nonce, and key-generation number;
key bytes are never serialized, placed in a checkpoint, included in a receipt, or
written to diagnostics. Rotation is staged and resumable: the previous generation is
retained until every replacement decrypts and verifies, and only an explicit commit
retires it. A lost key, compromise response, or user reset requires a confirmation whose
scope is either one generation or all locally retained generations. The resulting
receipt states exactly which generations were destroyed and that ciphertext under
those generations is unrecoverable. There is no cloud recovery secret or plaintext
queue fallback.

## Consent and scope

When a selected-root collector is enabled, consent must state the source, selected
scope, exclusions, private-context behavior, fields retained, policy version, and
how to stop or delete the journal. The collector requires a rendered and explicitly
confirmed preview before `start`; a source cannot expand scope because a path
contains a symlink, a mounted volume, a browser profile, or a process-owned file.
Scope checks and canonicalization occur before a value reaches the journal.

Consent is not a substitute for minimization. A user selecting a root does not
authorize file contents, keystrokes, screenshots, clipboard values, page contents,
or unrelated application data.

Consent is represented as an append-only local state machine. Grant, scope change,
suspension, revocation, and deletion intent each produce a bounded receipt containing
the policy identity and version, a SHA-256 scope digest, time, actor code, and reason
code. Receipts never contain selected roots or observations. Revocation changes the
capture gate synchronously; cleanup may run afterward but cannot re-enable retention.
Receipt replay requires contiguous sequence numbers and valid state transitions, and
only an explicit grant can return a non-active state to active.

Before a live root is eligible, `ConsentPreview` renders the canonical opaque root
identities, exclusions, retained fields, and known coverage limits. The caller must
explicitly confirm that preview before `grant_preview` can create an active receipt;
preview contents are bounded and are not persisted in the receipt. A revoked or
deletion-requested state is terminal for the current observation session and is
reported before cleanup can proceed.

Policy decisions expose a finite outcome set (`allow`, `deny`, `redact`, `summarize`,
or `refuse`) and finite diagnostic classes. Public decision records contain policy
identity/version, source, root presence, private-context state, and a reason code;
they do not contain the root string or rejected observation. Malformed input,
unsupported scope, and internal failure remain distinguishable without echoing the
input that caused the refusal.

The policy document carries selected roots and an independent bounded exclusion set.
An excluded root always loses to a selected-root grant and produces the stable
`root_excluded` reason. Both sets are included in the scope digest and are treated as
semantic migration changes, so a changed exclusion requires reconfirmation. The
document parser accepts the omission of this optional v1 field for backwards
compatibility, while duplicate, malformed, or oversized values remain fail-closed.

The versioned exclusion matcher adds root, subtree, file-kind, application,
temporary-file, VCS, and user-pattern rules without retaining the matched value in
diagnostics. Deny is always stronger than redact, summarize, or allow; rule class
and literal specificity then resolve overlaps. Empty, traversal, malformed-escape,
and oversized patterns are rejected before a policy can be installed. Policy history
keeps the old version available for existing evidence and evaluates only future
observations against a newly installed version.

The selected-root collector stores only an opaque root ID, operation, entry kind, path
class, and SHA-256 path digest. It does not open, read, or hash file contents. Containment
uses the operating system's canonical path plus device/inode identity, so parent traversal,
lexical-prefix tricks, and different-device descendants are refused. Case and Unicode
equivalence are not invented by the collector: composed/decomposed and case variants are
equivalent only when the selected filesystem resolves them to the same identity. Digest
bytes are scoped to the opaque root ID and its filesystem identity, so an exported digest
is stable only within that documented normalization and scope boundary. Lifecycle
transitions, callback health, blocked counts, and callback overflow gaps are bounded status
records; the raw callback path is never written to a payload or diagnostic. If a later
consumer must open a path, `SelectedRoot::open_contained` uses a descriptor walk with
no-follow component opens, rejects symlink replacement and regular-file hard-link
aliases, and exposes only descriptor metadata. Symlink and hard-link callback flags are
retained as source facts without opening their targets. Volume identity adds only
device/filesystem fields and an optional volume-UUID digest; mutable display names
are excluded. Cursor identities require the same volume and stream mode before a
resume is allowed. The durable cursor boundary stores only root and exclusion
digests plus bounded stream settings (`since_when`, latency, and file-event mode);
it never stores a path or display name. Selected-root source-loss gaps add only a
volume fingerprint, opaque root IDs, bounded cursor range, stable reason code, and
remediation; they never retain the callback path or a rescan listing. Any boundary
change fails closed until an explicit reset or wrap. The collector exposes
`recovery_required` and refuses ordinary delivery after a loss until reconciliation;
ambient capture still requires the remaining release gates. Startup cursor
validation refuses zero, stale, future, wrapped, and corrupted positions instead
of silently dropping history. Historical replay remains `Replaying` until the
path-free `HistoryDone` control boundary; timeout, partial-history status, or an
explicit stop before that boundary records only a bounded history gap and keeps
the collector unavailable for ordinary delivery. The sentinel is never written as
an observation, and no source path is included in the refusal or gap text.

## Local storage and export

The intended journal is local to the user account. Filesystem permissions, SQLite
WAL sidecars, backups, crash dumps, and operating-system indexing can expose
metadata even when payloads are encrypted. The production design therefore treats
the key, plaintext buffers, export destination, and temporary files as separate
assets.

The file-backed path boundary is fail-closed: the journal directory is created and
verified as user-owned mode `0700`; the database and SQLite sidecars are verified as
user-owned, single-link regular files mode `0600`; and exports and temporary output
files are forced to mode `0600`. No-follow opens and identity rechecks reject a
symlink, hard-link substitution, non-regular file, unsafe mode, or parent replacement.
The selected-root collector separately denies source notifications that resolve to
the journal, its sidecars, or caller-registered export/backup/temporary paths before
hashing or persistence. Existing internal objects remain denied after relocation by
device/inode identity; symlink redirects are canonicalized and fail closed. An
internal denial is represented only by a bounded `internal_storage_path` summary,
never by a raw path or a filesystem payload. The CoreServices `OwnEvent` flag is
retained as source evidence for unrelated paths and is not an unconditional drop
rule.
These checks reduce local path-confusion risk but cannot prevent a privileged process,
filesystem snapshot, crash dump, or a user from copying an explicit export.

The active WAL has an explicit privacy and resource policy: automatic checkpoints,
bounded busy waits, a maximum read-transaction lifetime, and a maximum sidecar size.
Passive checkpoints report remaining frames and refuse when the configured bound is
not met. A database snapshot requires a truncate checkpoint and copies only the
database file; `-wal`, `-shm`, rollback-journal, temporary, and backup sidecars are
never accepted as standalone backups.

Export is an explicit user action. The CLI refuses to overwrite an existing
destination without --force. An export may contain sensitive plaintext and is the
user's responsibility once written outside the protected journal directory.
GHOSTRACE does not upload or fetch a destination.

The optional Parquet archive profile is a contract for a future user-requested
derived copy, not an automatic export path. `ghostrace parquet-profile` prints the
checked-in strict v1 profile and never reads a journal or creates an archive. The
profile keeps the plaintext boundary explicit, requires mode `0600` temporary files
and atomic publication, and requires cleanup on failure. A writer must disable
Parquet dictionary encoding, column statistics, and page indexes: compression can
reduce size but does not provide confidentiality, while statistics and metadata can
leak values or ranges even when payload columns are encrypted elsewhere. The profile
records that Parquet encryption is not assumed, so a downstream consumer must treat
the archive as sensitive plaintext unless it establishes its own encryption boundary.
Profile validation rejects undeclared columns, lossy conversions, non-canonical JSON,
oversized rows, and missing essential gap fields rather than silently coercing them.

The archive is a separate external copy. Journal retention and key destruction do
not erase it, and GHOSTRACE cannot retract copies made by downstream tools or
backups. Any future deletion feature must identify the copy owner and scope, report
what remains recoverable, and preserve the source journal when archive publication
fails.

### Explicit shell wrapper boundary

The shell metadata contract is intentionally narrower than a terminal transcript.
`ghostrace shell-schema` only prints the checked-in v1 contract and never executes
a shell. A future wrapper may retain the opaque wrapper session, normalized
executable identity, sanitized working-directory class/digest, start/end timing,
outcome, exit code, and termination signal. These fields are identifying,
sensitive metadata, timing evidence, or outcome evidence as declared by the field
registry; they are not proof of command intent or complete process history.

Arguments, environment variables, standard input/output, shell history, aliases,
raw or expanded command text, and raw working-directory paths are not schema
properties and are rejected by both JSON Schema and typed deserialization. Invalid
tokens, credential-like executable identities, reversed timestamps, inconsistent
exit/signal combinations, and oversized metadata fail closed without echoing the
untrusted value. This contract does not authorize ambient shell capture; an
explicit wrapper and a later policy/consent gate remain required.

The lifecycle contract is tested only with fixed synthetic scripts in an offline
reference harness. The tests clear the child environment and connect standard input,
output, and error to null; they retain no command text, arguments, environment values,
or terminal bytes. Timeout, cancellation, terminal closure, and wrapper crash paths
remain explicit outcomes or gaps, and a gap never receives a fabricated completion,
end time, or success status.

The [`shell-secret-leakage-v1.json`](../fixtures/shell-secret-leakage-v1.json)
red-team corpus uses deterministic synthetic sentinels to exercise every denied
shell channel and checks the metadata validator, journal bytes, diagnostics, errors,
exports, CLI output, and panic output for retention or echo. The process-inspection
and crash-report rows are intentionally separate: an operating system inspector or
crash reporter can observe synthetic process state outside GHOSTRACE's retention
boundary. GHOSTRACE makes no claim to hide those values from the operating system;
it only verifies that the application does not retain them.

## Retention and deletion

The fixture headstart has no ambient retention burden. The read-only
`retention-plan` command is the first retention boundary: its documented default
is observations older than 90 days, anchored at an explicit UTC `as_of` time. A
caller can instead supply a UTC cutoff, source, opaque filesystem root, maximum
newest event count, or maximum newest encrypted-payload bytes. Scope filters are
intersected first; gap preservation is enabled by default; time, event-count, and
byte selectors then union their oldest eligible rows. The plan reports a stable
observed/ingested range, affected sources, ciphertext key generations, gap
summaries, a conservative encrypted-payload byte estimate, and an authenticated
candidate-set digest.

The plan is evaluated inside one SQLite read snapshot and binds its committed
`ingest_seq` upper bound. Its confirmation contains only the plan digest,
candidate digest, and boundary. A later deletion command must refuse any set that
would exceed that boundary or differ from that digest, so concurrent ingest or a
changed policy cannot expand a previously confirmed scope. The plan is not a
deletion command and does not infer consent from a prior export.

Retention deliberately excludes exports, database backups, SQLite WAL/SHM
sidecars, diagnostic records, cursors, and encryption-key references from this
journal-event scope. Legal holds are not implemented and are never inferred from
an export or backup. Future deletion work must define residue, transactional
behavior, recovery, and what remains recoverable from external copies before it
can remove journal rows or related artifacts.

### Residue modes and erasure limits

`residue-report` is a read-only, path-free inventory. It makes four operations
explicit so a future deletion command cannot present one as another:

- **Logical deletion** removes selected rows from the live SQLite view. It is the
  least expensive operation, but free pages, WAL frames, virtual-table shadow
  tables, backups, and filesystem copies can remain.
- **Compaction** checkpoints and may run `VACUUM` to rewrite the live database.
  It costs I/O and temporary space, can be refused by readers, and is not a
  secure-erasure primitive. It does not rewrite external copies or SSD media.
- **Cryptographic erasure** destroys the journal key generation. Authenticated
  ciphertext can remain in database, WAL, shadow, or backup bytes, but a keyless
  reader cannot decrypt it. Plaintext exports and independently encrypted copies
  are not covered.
- **External-copy handling** removes known exports/backups only when their owner,
  permissions, and media allow it. Filesystem snapshots, Time Machine, cloud
  sync, offline media, wear levelling, and privileged recovery are outside this
  process's inventory.

SQLite `secure_delete` is reported rather than treated as a guarantee. SQLite
documents limits around virtual tables and shadow storage; `VACUUM` changes the
live file layout but cannot promise erasure from WAL/SHM, backups, snapshots, or
the underlying storage device. The UI/CLI must use the same mode labels and show
unsupported-media limits instead of displaying a generic "secure delete"
success.

### Transactional deletion and integrity recovery

`retention-delete` accepts only a matching plan digest, candidate-set digest, and
snapshot boundary. It takes an immediate SQLite transaction, re-evaluates the
candidate identities, refuses scope changes, durable cursor tails, and
unselected child references, then deletes rows in reverse ingest order. A
failure before commit leaves the journal unchanged. The receipt explicitly says
that compaction was not performed and external copies were untouched.

`integrity-check` runs SQLite integrity and foreign-key checks without repair.
When it fails, ingestion must stop, the original database and sidecars must be
preserved, and recovery must happen on a private verified copy with before and
after receipts. A successful check is not proof of deletion or external-copy
erasure.

### Authenticated state and deletion boundaries

The authenticated-state anchor stores only keyed digests, counts, schema/key
generations, chain boundaries, and a deletion digest. Canonical inputs include
event metadata and ciphertext bytes, cursor fields, policy-history JSON, and
bounded diagnostic fields; they never include key bytes, plaintext payloads,
filesystem paths, or retained event identifiers. The local key is used only to
authenticate the anchor and is never serialized.

The verifier reports edits, insertion/deletion, reorder or truncation, cursor
rollback, policy substitution, diagnostic tampering, and missing/invalid
anchors. Retention deletion advances a chain epoch and records only the
confirmed plan/candidate digests, snapshot boundary, and counts. A valid result
means possession of the configured local key authenticated the local state; it
does not prove collection origin, completeness, remote identity, or legal
chain of custody. A missing anchor after bootstrap is a stop signal and cannot
be reseeded automatically.

## Privacy verification

Privacy changes require tests that:

- prove prohibited fields are absent from accepted events and exports;
- prove private contexts are rejected by default;
- prove paths outside selected roots and symlink escapes are denied;
- prove rejected values do not appear in errors or diagnostics;
- run the fixture path without network access;
- show gaps and denials in explanations;
- check that logs never contain keys, payloads, or user paths.

The consent/policy state-machine corpus is deterministic and dependency-free. It
exercises 512 policy matrices and 256 consent command sequences, including
exclusion precedence, redaction, versioned migration, receipt replay, contiguous
sequence numbers, monotonic timestamps, failed-command immutability, and rejection
of forged non-grant reactivation. The fixed generator makes failures reproducible on
the target device without downloading a test framework.

The exclusion corpus additionally covers nested and case-variant paths, escaped
wildcards, every rule class and outcome, order-independent digests, malformed and
empty patterns, versioned future-only updates, and repeated maximum-size matching.

See [EVALUATION.md](EVALUATION.md) for the evidence expected before a live source is
enabled.
