# Task 0066 evidence: volume identity and mount transitions

Status: implementation complete on the source branch; final protected-main
device receipts are added after PR merge.

## Contract

`VolumeIdentity` contains a contract version, operating-system device number,
filesystem identity, and an optional SHA-256 digest of a platform volume UUID.
It contains no volume display name or path. `VolumeIdentity::from_path` reads
only filesystem metadata and an opaque `statfs` filesystem ID; mount names are
never interpreted or retained. A caller that has a platform volume UUID can
provide only its digest.

`CursorIdentity::for_volume` binds a live cursor to a `VolumeIdentity` and an
explicit `CursorStreamMode` (`per_host` or `per_device`). `can_resume_from`
requires source, collector instance, stream mode, and volume identity to match;
a matching path string or collector name cannot authorize a different volume.
Fixture identities remain separate and cannot be used as live volume cursors.

`VolumeObservation` and `VolumeTransition` provide path-free continuity
classification. Unmount, mount, remount, device replacement, APFS snapshot
restore, and path reuse are explicit outcomes. The selected-root collector
retains volume identity in memory and includes its fingerprint in the path
digest scope. Durable cursor persistence is intentionally left to task 0070 /
0015, which will persist this identity with the replay boundary.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Stable identity fields and no mutable display-name sole identity | `VolumeIdentity` serializes only contract version, device ID, filesystem ID, and optional UUID digest; `volume_identity` tests assert display names and mount paths are absent. |
| Mount, unmount, replacement, snapshot restore, and path reuse discontinuities | `tests/volume_identity.rs` exercises each transition and asserts `VolumeTransition` plus discontinuity status. The snapshot case uses a distinct snapshot digest and the path-reuse case keeps the scope digest while changing volume identity. |
| Cross-volume cursor reuse is refused | `tests/volume_identity.rs::cursor_resume_requires_volume_and_stream_mode_match` uses the same source, collector instance, and path-equivalent scope while changing the volume or stream mode; `can_resume_from` refuses both. |

## Limits

This task defines and tests the boundary; it does not enable live FSEvents
cursor persistence, mount notification privileges, APFS snapshot enumeration,
or external-volume hardware tests. The current device receipt is therefore a
deterministic contract and metadata run, not a claim that detach or snapshot
hardware was exercised. Durable storage and recovery remain later gates.

## Target-device verification

The final protected-main receipt, exact commit, device details, and log digests
are retained in this document after the evidence PR is merged. No path,
display name, account data, credential, or capture key is retained.
