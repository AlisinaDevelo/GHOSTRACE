# Task 0066 evidence: volume identity and mount transitions

Status: complete on protected `main` at
`167f17480f85935c9d2ba1b3bd31b63265e36c33` (PR #250). Evidence is retained by
PR #251 after the device lanes below passed on that exact commit.

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
digest scope. Task 0070 now persists this identity with the replay boundary;
full restart recovery and source-loss repair remain task 0015 and its children.

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

The verification below ran on the protected-main checkout, with no network
required for the offline lane:

| Lane | Command | Result | Receipt SHA-256 |
| --- | --- | --- | --- |
| Focused contract | `cargo +1.88.0 test --locked --lib fsevents_collector::tests`; `cargo +1.88.0 test --locked --test volume_identity`; `cargo +1.88.0 test --locked --test selected_root_scope`; `cargo +1.88.0 test --locked --test cursor_contract` | pass (10 + 4 + 2 + 5 tests) | `caa1843d46117672cc379eb556c49f94a2a0984ef8cb71f76b1b7d055519022e` |
| Full debug | `cargo +1.88.0 test --locked --all-targets --all-features` | pass | `e80d59cf3da96f7b63248cff68566853670cef4e734e5ce70c4381778c78b0fd` |
| Full release | `cargo +1.88.0 test --release --locked --all-targets --all-features` | pass | `2f121dc014fb15a199a4068e2bc5583e7937b0a3d1eb28037f9abba09ba6593a` |
| Reproducibility | `CARGO_NET_OFFLINE=true /bin/bash scripts/reproducibility-test.sh` | pass; all checks passed | `248522f5fd4dccb8225f0bfd2f1ccd7b74d1d6bf988b3d486e96410481cf466b` |
| Rust documentation | `RUSTDOCFLAGS=-D warnings cargo +1.88.0 doc --locked --all-features --no-deps` | pass | `1afef1fda8229816076e65d6121d7e119eddd1eacec4a3345bab1c8d6df22556` |
| Offline network | `scripts/offline-network-test.sh` | pass | `7f76f21791071e72787a1fbc4fe20d7161d595d9ebc4c5d45a85ec8c64ccc047` |
| Python policy/roadmap tests | `python3 -m unittest discover -s tests -p 'test_*.py'` | pass; 40 tests | `137f35433abf8e1702d65166a008e1b16e8f84865aa39783b40d5a98fdf04dd4` |

Device: macOS 26.6.2 (25G83), Darwin arm64, MacBookPro17,1, Apple M1;
`rustc 1.88.0 (6b00bc388 2025-06-23)`, target `aarch64-apple-darwin`.
Hosted PR #250 checks also passed for macOS stable, Linux stable/MSRV,
clippy, rustfmt, offline fixture, audit, dependency review, and roadmap.
No path, display name, account data, credential, or capture key is retained.
