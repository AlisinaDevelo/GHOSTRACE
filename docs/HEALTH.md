# Offline health inspection

`ghostrace health --journal <existing-journal>` prints a local human-readable
report. Add `--json` for the strict [v1 schema](../schemas/health-report-v1.json).
It accepts fixture or live journal storage without a journal key. A readable
report exits 0; a storage, format or query-budget refusal exits 1 with the report
on stdout and no underlying error on stderr.

The output contains only fixed status/reason/remediation codes, the binary's
package version, the validated journal schema version, and aggregate event, gap, cursor, policy-version and diagnostic
counts. It does not expose paths, event IDs, timestamps, origins, commands,
titles, policy bodies, diagnostic details, ciphertext or credentials. Human
remediation text is fixed; it never incorporates input or database text.

`available_unverified` means metadata could be read, **not** that it is authentic
or intact. Key access, collector/service liveness, OS permissions and update
availability remain `not_checked`. No missing observation is invented as a
healthy status. Failed reads return `counts: null`, not zeroes. Coverage gaps
and inactive/unknown cursors receive explicit attention codes. No-policy-records
is a review signal, not authorization to start a collector.
An empty journal reports `no_events_observed` and `no_cursor_records` as
`not_checked`, rather than treating absence of evidence as healthy coverage.
Unavailable schema versions remain null. No identity is invented or copied from
unauthenticated journal metadata.

The database is opened read-only with the existing private-storage checks, a
single read snapshot, query-only mode and untrusted-schema restrictions. There
is no migration, checkpoint, repair, payload decryption, Keychain access,
service/browser connection, permission request or network call. SQLite may
create empty WAL/SHM coordination sidecars when opening WAL storage read-only;
this is not a new event or a database write. Missing storage is checked before
the storage helper can prepare a parent directory.
Read-only storage never creates a missing parent. File and parent identities
are compared before and after SQLite opens the path, with the verified file
descriptor kept open. These checks detect replacements; they are not a promise
of protection against a hostile same-user process controlling the filesystem.

The migration ledger, checksums, schema/version rows and required table-column
contracts must match the compiled catalog. The expected column contract is
built in a separate in-memory database; the input journal is never migrated.
Ledger text is byte-bounded (including embedded NULs) and numeric fields must
have SQLite integer storage types before any records are loaded.
This is still only structural readability, not authentication or a full
integrity check. Corrupt or interrupted current ledgers are recovery signals; older/future schema versions require
a compatible version, and contention receives `storage_busy` with a retry code.

SQLite busy waits are capped at 250 ms, the page cache at 2 MiB, and query work
at 2,000,000 VM instructions or two seconds checked every 1,000 instructions.
Before the first query, connection-local SQLite limits cap string/BLOB values
and SQL text at 64 KiB, table/result columns at 32, and expression depth at 64.
Fields used in comparisons have narrower type/byte guards. These are separate
from the VM callback: one large value must not evade the instruction budget.
Known SQLite sidecars must total at most 64 MiB before opening, and that bound
is checked again around the open. Oversized inputs receive
`storage_budget_exceeded` with no counts and preservation/recovery remediation;
they are not recovered or truncated.
Counts deliberately use `COUNT(1)` rather than a fast single B-tree Count
opcode. This is a cooperative query budget, not a hard deadline for an operating
system file operation stalled below SQLite or a whole-process heap cap.
Large journals may be refused;
the tool neither increases its budget silently nor reports a partial count.

This command is not a support-bundle export, a privacy/security audit, a runtime
health probe, or a v1.1 operational-readiness claim. Task 0126's report
implementation does not satisfy its prerequisite v1 release gate 0040.
