# Platform policy

GHOSTRACE is macOS-first because its first live source and its future protected key
store are macOS facilities. The current fixture core is intentionally portable
enough to build and test on Linux; portability must not weaken the macOS privacy
boundary.

## Support posture

| Area | Current statement |
| --- | --- |
| Operating system | macOS 15.0 (Sequoia) is the planned development floor for live capture; the release gate will revalidate it and may raise it |
| Architectures | Apple silicon and Intel are design targets; each must be tested against the supported macOS floor before release |
| Fixture core | Linux and macOS CI exercise parsing, policy, storage, explain, and export without ambient capture |
| Distribution | No signed, notarized, or bundled release artifact is shipped in the headstart |
| Permissions | The fixture path requests none; live permissions are a future, explicit decision |

The complete, machine-readable support and permission contract is in the
[support matrix](SUPPORT_MATRIX.md). It records target versus verified macOS
major-version/architecture rows, explicit unavailable-hardware no-go rows, and
the required/optional/prohibited permissions plus observable refusal for every
planned collector.

## Journal file boundary

The local journal directory is user-owned and exactly `0700`. Database, WAL, SHM,
rollback-journal, temporary, backup, and explicit export files are regular,
single-link, current-user-owned files with mode `0600`. Creation is component-wise,
uses no-follow open flags, and rechecks parent and file identities around the SQLite
open. Symlinks, non-regular files, hard-link anomalies, foreign ownership, unsafe
modes, parent traversal, and path-component replacement fail closed. These checks are
storage invariants, not a substitute for the future signed/notarized distribution
and Keychain entitlement gates.

File-backed journals use the WAL policy in [ADR 0003](adr/0003-sqlite-wal-active-journal.md):
bounded busy waits and read snapshots, automatic checkpoints, and an observed sidecar
limit. A long reader or over-limit WAL produces a bounded refusal. Database snapshots
are made only after a truncate checkpoint and never by copying a `-wal` or `-shm`
sidecar independently.

Before a journal is used, its embedded migration catalog is checked against the
on-disk ledger. The ledger binds stable migration identifiers and SHA-256 SQL
checksums to schema versions and tool versions. Upgrade steps are transactional;
missing, changed, reordered, future, partial, and unsupported-downgrade states are
explicit refusals. Crash recovery and database-only backup restore are part of the
device evidence, not inferred from a successful CI build.

## Permission boundary

The baseline does not require root, Full Disk Access, Accessibility, or Automation.
It does not install a privileged helper. A future collector may request only the
minimum permission required for the selected source, explain why it is needed, and
remain disabled when consent is absent or revoked.

Endpoint Security is optional and entitlement-gated. It is not a hidden fallback for
FSEvents and will require its own threat model, attribution tests, user-facing
consent, and release evidence.

## Keychain constraints

The journal wrapping key is scoped to the macOS data-protection Keychain. The provider
uses the `com.alisinadevelo.ghostrace.journal` service and
`journal-wrapping-key-v1` account, disables iCloud synchronization, and requires
`WhenUnlockedThisDeviceOnly` access control. The default application uses no access
group. A command-line helper or extension must be code-signed with the same bundle
identity and any explicitly configured Keychain access group; otherwise the provider
reports a bounded failure. Key reads are available only in an unlocked user login
session, and no legacy-keychain fallback is permitted.

## FSEvents boundary

The planned first live source observes bounded filesystem metadata below explicitly
selected roots. It must canonicalize roots, reject symlink escapes, apply exclusions
before persistence, and retain no file contents. FSEvents does not guarantee
process attribution, event completeness, or one notification per change. Source
flags, cursor state, and gaps must remain visible.

The shipped lifecycle adapter is only the native stream fence. It requires a
single owner thread and that thread's current Core Foundation run loop; it does not
start ambient capture by itself. Creation, scheduling, callback parsing, flush,
stop, restart, invalidation, and release are explicit operations. A callback is
never allowed to unwind through the C ABI, and a native stream is released only
after invalidation. The separate selected-root consent preview now makes canonical
opaque roots, exclusions, retained fields, and FSEvents coverage limits explicit
before a grant; canonical filesystem-path resolution, symlink containment,
exclusions, source flag semantics, cursor recovery, and persistence remain no-go
gates for a live collector.

The adapter rejects FSEvents callback modes that replace the raw C-string path
array with CFType or extended-data values (including full-history and document-ID
modes); those modes require a separate parser contract. The raw callback flag word
is normalized immediately after copying: all 23 documented event bits map to
`FseventsEventFlag`, dropped or wrapped coverage becomes an explicit rescan status,
and root/mount/history markers remain boundaries. Unknown bits are preserved as a
numeric remainder and lower completeness; contradictory item-kind or mount-state
combinations are refused without discarding the raw word. The normalized v1 record
contains no path and therefore cannot bypass root canonicalization or privacy
policy.

## Private contexts

Private browsing and private application contexts are excluded by default. A future
browser or frontmost adapter must define how it detects private context and must not
turn it on merely because a user selected a filesystem root.

## CI and cross-platform work

Linux CI provides a fast, deterministic test environment for the fixture contract.
macOS CI is required for platform APIs, permissions, Keychain integration, and
FSEvents behavior. Platform-specific code belongs behind an explicit adapter boundary
so compiling the fixture core cannot accidentally enable live capture.

The [roadmap](ROADMAP.md) and [evaluation plan](EVALUATION.md) define the evidence
required before the support matrix becomes a release promise.
