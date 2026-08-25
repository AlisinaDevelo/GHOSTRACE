# Task 0065 evidence: FSEvents flag normalization

Status: complete for the fixture-only normalization contract. This gate does not
enable live collection, request permissions, open paths, or write a journal.

## Contract

`src/fsevents_flags.rs` defines `fsevents-normalized-v1`. The normalizer retains the
raw `u32`, emits every documented Apple event bit in canonical numeric order, and
retains future bits in `unknown_bits`. An unknown bit produces an explicit
`unsupported` status and `lowered` completeness. Buffer loss and wrapped cursors
produce `rescan_required`; mount/root/history markers remain `boundary`. Contradictory
item-kind or mount-state combinations are `contradictory`. The normalized record is
path-free so root canonicalization and privacy policy cannot be bypassed.

The source of the flag names and values is Apple's
[FSEventStreamEventFlags reference](https://developer.apple.com/documentation/coreservices/1455361-fseventstreameventflags).

| Flag | Value | Canonical meaning |
| --- | ---: | --- |
| `MustScanSubDirs` | `0x00000001` | A recursive rescan is required. |
| `UserDropped` | `0x00000002` | Client-side buffering dropped coverage; rescan. |
| `KernelDropped` | `0x00000004` | Kernel-side buffering dropped coverage; rescan. |
| `EventIdsWrapped` | `0x00000008` | Event ID continuity wrapped; rescan/cursor repair. |
| `HistoryDone` | `0x00000010` | Historical delivery boundary reached. |
| `RootChanged` | `0x00000020` | Watched root changed; boundary and lowered completeness. |
| `Mount` | `0x00000040` | Mount boundary. |
| `Unmount` | `0x00000080` | Unmount boundary. |
| `ItemCreated` | `0x00000100` | Item creation operation. |
| `ItemRemoved` | `0x00000200` | Item removal operation. |
| `ItemInodeMetaMod` | `0x00000400` | Inode metadata changed. |
| `ItemRenamed` | `0x00000800` | Item rename operation. |
| `ItemModified` | `0x00001000` | Item content or metadata modified. |
| `ItemFinderInfoMod` | `0x00002000` | Finder information changed. |
| `ItemChangeOwner` | `0x00004000` | Ownership changed. |
| `ItemXattrMod` | `0x00008000` | Extended attributes changed. |
| `ItemIsFile` | `0x00010000` | Item is a regular file. |
| `ItemIsDir` | `0x00020000` | Item is a directory. |
| `ItemIsSymlink` | `0x00040000` | Item is a symbolic link. |
| `OwnEvent` | `0x00080000` | Event originated from this client. |
| `ItemIsHardlink` | `0x00100000` | Item is a hard link. |
| `ItemIsLastHardlink` | `0x00200000` | Item is the last hard link. |
| `ItemCloned` | `0x00400000` | Item was cloned. |

## Golden callback batches

The integration test covers a normal compound item, user and kernel drops with a
required scan, an event-ID wrap, a history marker, a contradictory file/dir pair,
and a future high bit. It asserts exact serialized output, canonical flag ordering,
schema validation, unknown-bit retention, and path-free normalized output. The unit
tests additionally prove the documented mask has exactly 23 unique bits.

## Target-device receipts

The implementation and receipts below were run on 2026-08-25 from protected
`main` commit `40b745e9580cf6d02d13ac6c8863d8d228db9415` on a MacBook Pro 17,1
(Apple M1, 8 GB, arm64; macOS 26.6.2 (25G83); Rust/Cargo 1.88.0):

| Check | Result and receipt |
| --- | --- |
| Focused flag integration tests | 4 passed, including the strict schema and golden batches. |
| Full release all-target/all-feature suite | Passed, exit 0; `/private/tmp/ghostrace-0065-release-all-v1.log`; SHA-256 `597da6e5a7dc3a4fd0fc87311b282db5e172b8b05e3aecbdfc0c05a0e7e5e219` |
| Enforced macOS sandbox canary plus FSEvents/privacy focus | Passed, exit 0; `/private/tmp/ghostrace-0065-sandbox-v1.log`; SHA-256 `96dae96edef563090365f5500a3a939088cedb1ddca801333ec98a3e8855178e` |
| Static/repository lane | Passed, exit 0; `/private/tmp/ghostrace-0065-static-v1.log`; SHA-256 `b3489737dd130246d21d7277ba1d2cecd500f47c72ff24d886f696663d3939ba` |

The static lane covered formatting, locked release Clippy with `-D warnings`,
fixture/identity/release-evidence/roadmap/reproducibility checks, generated-index
parity, 38 Python tests, ShellCheck, actionlint, and a source-only product
network-surface scan. A separate full debug all-target attempt was not a test
result: it exited 101 before compilation because the device filesystem reached
`ENOSPC`; its generated target was removed before the successful release run.

## Limitations

- The normalizer is a bounded source primitive, not a filesystem walk or symlink
  containment check. Those remain selected-root gates.
- A rescan status records the obligation; it does not claim that a future rescan has
  succeeded.
- Docker is not available on the target device; the hosted Linux network-denial lane
  remains the authority for that environment.
