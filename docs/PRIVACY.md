# Privacy model

GHOSTRACE exists to help a person inspect a bounded sequence of local changes. It is
not designed to reconstruct everything a person did. The current public headstart is
fixture-only, local, and offline; no live collector is enabled.

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
| Selected path metadata | Describe a permitted filesystem change without reading content | Future, explicit root only | Canonicalized, excluded, and bounded before persistence |
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

## Consent and scope

When live capture is eventually enabled, consent must state the source, selected
scope, exclusions, private-context behavior, fields retained, policy version, and
how to stop or delete the journal. A source cannot expand scope because a path
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

Policy decisions expose a finite outcome set (`allow`, `deny`, `redact`, `summarize`,
or `refuse`) and finite diagnostic classes. Public decision records contain policy
identity/version, source, root presence, private-context state, and a reason code;
they do not contain the root string or rejected observation. Malformed input,
unsupported scope, and internal failure remain distinguishable without echoing the
input that caused the refusal.

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

## Retention and deletion

The fixture headstart has no ambient retention burden. Live retention and deletion
commands are roadmap work. They must define whether deletion covers WAL files,
backups, exports, diagnostic records, cursors, and encryption-key references, and
must report what cannot be recovered from an external copy.

## Privacy verification

Privacy changes require tests that:

- prove prohibited fields are absent from accepted events and exports;
- prove private contexts are rejected by default;
- prove paths outside selected roots and symlink escapes are denied;
- prove rejected values do not appear in errors or diagnostics;
- run the fixture path without network access;
- show gaps and denials in explanations;
- check that logs never contain keys, payloads, or user paths.

See [EVALUATION.md](EVALUATION.md) for the evidence expected before a live source is
enabled.
