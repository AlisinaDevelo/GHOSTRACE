# Threat model

This model covers the local fixture headstart and the future opt-in macOS journal.
It is a design boundary, not a claim that every implementation risk has been
eliminated. GHOSTRACE records diagnostic evidence; it does not create legal chain of
custody.

## Security objectives

1. Keep collection within the user's explicit scope.
2. Minimize what enters the journal and reject prohibited data before persistence.
3. Preserve provenance, ordering limits, and gaps without overstating causality.
4. Protect production payloads and keys at rest.
5. Make exports and other plaintext transitions deliberate and visible.
6. Keep the baseline local, offline, and free of silent upload paths.

## Assets

- event payloads, paths, timestamps, source cursors, and policy state;
- encryption keys and plaintext buffers;
- journal database, WAL and sidecar files, backups, and exports;
- evidence links, event ordering, gaps, and coverage claims;
- user consent, exclusions, and private-context state;
- build and dependency integrity.

## Actors and assumptions

| Actor | Capability or motivation | Trust assumption |
| --- | --- | --- |
| User | Reads, deletes, exports, or changes local files | The user controls their account and can stop the process |
| Same-user process | Can often read user-owned files and observe process behavior | Not trusted; same-user compromise is a residual risk |
| Other local user | May access only what operating-system permissions allow | Not trusted |
| Malicious fixture or path | Supplies malformed, huge, ambiguous, or attacker-shaped input | Not trusted |
| Malicious dependency or build input | Attempts to change behavior or add a network path | Controlled by review, lockfile, advisories, and policy checks |
| Remote attacker | Has no intended application network channel | Relevant only through compromised dependencies, exports, or the host |

The host operating system and macOS security services are assumed to enforce their
documented permission model. This assumption does not make FSEvents complete or
protect against a compromised same-user account.

## Trust boundaries

~~~text
untrusted fixture / OS observation
              │
              v
     parser + bounded normalizer
              │
              v
       consent / policy gate
              │
              v
      event envelope + provenance
              │
              v
        journal writer + key
              │
        ┌─────┴─────┐
        v           v
  local query   explicit export
~~~

The parser boundary protects the process from malformed input. The policy boundary
protects the journal from unauthorized fields and scope. The storage/key boundary
protects payloads at rest. The export boundary is a deliberate transition to
plaintext chosen by the user.

## STRIDE analysis

| Category | Example threat | Mitigation | State |
| --- | --- | --- | --- |
| Spoofing | A fixture or adapter claims another source or policy | Sealed typed origin capabilities, versioned provenance namespaces, policy IDs, and validation | Fixture and selected-root live-origin boundaries are tested; Endpoint Security attestation remains future work |
| Tampering | A local process edits journal rows or an export | Authenticated payloads; future integrity chain; explicit integrity status | Keychain encryption and chain verification are roadmap gates |
| Repudiation | An explanation hides a denied interval, restart, or replay conflict, or a callback is lost during native shutdown | First-class gaps, typed source identity, volume-bound stream mode, durable replay boundary, WatchRoot, stable loss reasons, bounded recovery metadata, explicit recovery gate, event IDs, policy binding, deterministic output, bounded callback queue, lifecycle records, named crash/replay matrix, owner-thread FSEvents shutdown fence | Selected-root lifecycle, blocked-summary, overflow-gap, root-change, loss-reason, volume-transition, boundary-mismatch, restart, and atomic rollback tests now; durable source-loss reconciliation and full recovery remain required |
| Information disclosure | Logs, WAL files, exports, or errors reveal paths or payloads | Minimized fields, path digests, no sensitive diagnostics, explicit export, file permissions | Selected-root payloads and diagnostics contain no raw paths or contents; production release storage hardening remains |
| Denial of service | Huge fixture, event storm, callback panic, or native lifecycle leak exhausts memory or leaves capture wedged | Bounded parser, bounded callback batches/paths, panic containment, single-owner lifecycle, bounded pending queue, writer admission, one emergency status reservation, input limits, bounded retries, visible loss | Task 0016's native-safe stress lane proves the pending cap, auditable overflow gap, writer status reservation, and `recovery_required` transition; larger cross-device throughput remains future work |
| Elevation of privilege | Collector asks for broad TCC access or follows a symlink outside scope | No root/Full Disk Access baseline, explicit selected roots, startup canonicalization, descriptor-backed no-follow walks, device/inode and volume checks, policy gate, path digests | Selected-root metadata, descriptor-open containment, volume identity, and durable boundary binding are shipped; source-loss recovery and ambient capture gates remain required |

## Residual risks

- A same-user attacker may read plaintext while a process is running, inspect
  exported data, alter local configuration, or delete the journal.
- The host may leak metadata through filesystem indexing, backups, crash reports,
  swap, or file-system snapshots.
- FSEvents may coalesce, delay, reorder, or omit changes and does not provide
  process attribution or completeness. An explanation cannot repair that limitation.
- Exact callback re-delivery can inflate an explanation if it is mistaken for a
  new source fact. The selected-root collector therefore uses a bounded,
  deterministic `(event ID, flags, path digest)` window and exposes only a
  path-free suppression counter; distinct source IDs remain distinct evidence.
- Journal, SQLite sidecar, export, backup, and temporary-file notifications can
  otherwise feed back into the collector. The bounded internal-path policy checks
  them before hashing or persistence, binds existing objects to device/inode
  identity across relocation, and fails closed on symlink redirects. Denials are
  retained as a path-free `internal_storage_path` policy summary; the policy does
  not blanket-drop unrelated `OwnEvent` source deliveries.
- A rename callback contains one redacted path digest, not an old-to-new pair.
  Rename output says `unknown` (or a future bounded `contextual` relationship)
  and has no inferred wire value, so temporal adjacency cannot manufacture a
  sensitive old path.
- The lifecycle adapter and selected-root collector cannot make an FSEvents callback
  complete or attributable. They retain normalized flags, lifecycle state, explicit
  overflow gaps, policy outcomes, volume transition metadata, and durable metadata,
  but volume-bound cursor persistence, recovery, and source completeness remain
  later gates.
- A user may intentionally export sensitive data to an insecure destination.
- A compromised dependency, toolchain, or build host can violate the local-only
  contract. CI checks reduce this risk; they do not prove source intent.
- The fault matrix still exercises one deterministic fixture key generation. The
  separate key-lifecycle boundary now models versioned envelopes, resumable
  rotation, and explicit loss/reset receipts; it does not claim that a live
  collector or signed Keychain helper is enabled.
- A malicious path, fixture, or source can consume resources unless every adapter
  honors bounds. The benchmark corpus keeps entry, file-byte, run-time, and journal
  growth limits explicit; it records CPU, memory, disk, and energy limitations rather
  than treating a fast run as a safety proof.

These risks are communicated, not silently accepted as evidence quality.

## Severity calibration

Treat a boundary violation, silent network path, unredacted sensitive field, key
exposure, or false completeness claim as high severity. Treat a reproducible crash,
unbounded resource use, or export overwrite as at least medium severity depending on
whether data loss or disclosure is possible. Cosmetic explanation wording is lower
severity unless it changes evidence level or hides a gap.

See [SECURITY.md](../SECURITY.md) for private reporting and
[PRIVACY.md](PRIVACY.md) for the data inventory.
