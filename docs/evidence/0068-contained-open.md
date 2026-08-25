# Task 0068 evidence: descriptor-backed selected-root opens

Status: complete on protected `main`. Implementation PR #246 merged at
`a072248bac44f5444de19a00c9e626d2c4e63f21` on 2026-08-25. The receipts below
were produced after that merge on the target Mac; hosted checks were merge gates,
not a substitute for the local replay.

## Contract implemented

Root selection canonicalizes the requested path, records the expected device/inode,
and then opens every root component through a no-follow descriptor walk. A root
replacement between those checks is an explicit `ContainedOpenRace` refusal.

`SelectedRoot::open_contained` reopens the selected root through descriptors,
checks its identity, and walks each relative component with `O_NOFOLLOW` and
`openat`. The directory descriptor opened for one component authorizes the next;
the code does not revalidate a pathname and then use it. Different-device
descendants, symlink components, special files, and regular files with multiple
hard links are refused. The returned `ContainedFile` exposes only descriptor
metadata and identity stability; it deliberately does not implement content
reads.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Root selection and later opens use no-follow containment | `src/fsevents_collector.rs::open_directory_nofollow` walks from `/` with `openat`, `O_DIRECTORY`, and `O_NOFOLLOW`; `SelectedRoot::from_canonical_path` compares the expected identity with the descriptor identity; `open_contained` performs the same check before the component walk. Unit coverage includes replacement between identity and descriptor open. |
| Link events preserve source facts without reading targets | `entry_kind_for` retains `ItemIsSymlink` as `EntryKind::Symlink`, while `ItemIsHardlink` remains in the normalized raw flag set. The link-fact unit test checks both. Symlink and hard-link paths are refused by `open_contained`; the metadata collector never calls that method and never reads callback content. Public integration coverage is in `tests/selected_root_open.rs`. |
| Adversarial replacement of every component is denied | The descriptor-walk unit test runs three independent fixtures and mutates the first directory, nested directory, and final leaf to an outside symlink immediately before each corresponding `openat`; all three return explicit refusal. Public tests additionally cover parent traversal, lexical sibling escapes, symlink aliases, hard-link aliases, and an ordinary descriptor's identity stability. |

## Device and retained receipts

Receipts were run on 2026-08-25 from the exact protected-main commit above on a
MacBookPro17,1 (Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0,
Rust/Cargo 1.88.0, target `aarch64-apple-darwin`, and Python 3.9.6.

| Lane | Result and receipt |
| --- | --- |
| Focused protected-main containment suite | Pass: 10 collector unit tests, 3 collector integration tests, 2 public open tests, and 2 Unicode/case scope tests; `/tmp/ghostrace-0068-postmerge-focused.log`; SHA-256 `673a1842cf286b37b40c382d3594505e9ef6eeb0995a19b7a87ae9b69e3b5b49` |
| Full reproducibility pipe | Pass: pinned inputs, fixture manifest, rustfmt/schema, deterministic fixture CLI/export, capture refusal, roadmap/evidence checks, 40 Python tests, and all debug Rust targets; `/tmp/ghostrace-0068-postmerge-repro.log`; SHA-256 `f533f198a0a23d753f49851868c60b282565414b9b008cf339e8a3758f40c6d7` |
| Release all-target/all-feature tests | Pass: 31 library tests and every integration target; expected locked-Keychain and network-canary tests remained explicitly ignored; `/tmp/ghostrace-0068-postmerge-release.log`; SHA-256 `f66ebe396f5b75f030561a6812d14c5eb5e96bbac29ab09b14546f3003561d68` |
| Rustdoc warnings denied | Pass with `RUSTDOCFLAGS='-D warnings'`; `/tmp/ghostrace-0068-postmerge-doc.log`; SHA-256 `8cac35c8af4fc167c0fa11c2cb756efd8e37239f75227876efb07b6e907c967d` |
| Offline/network-denial lane | Pass under `sandbox-exec`: denial canary, privacy regression, and complete offline locked product suite; `/tmp/ghostrace-0068-postmerge-offline.log`; SHA-256 `c2c48a5cf195753fd06ce2e1b102eb5031a3fb9519bcd143964977fb171d2e05` |

Before merge, PR #246 also passed duplicate hosted Linux stable/MSRV and macOS
stable suites, Clippy with `-D warnings`, rustfmt, roadmap, Cargo policy,
dependency review, advisories, and the network-denial fixture lane.

## Limits and next gates

The FSEvents metadata adapter remains content-free and does not open callback
paths. This task supplies the descriptor-backed boundary for later consumers;
there is no ambient file-content reader to enable yet. The race fixtures use a
deterministic pre-`openat` mutation hook rather than claiming a probabilistic
stress campaign. The mixed-device check is identity-based and does not require a
second external volume. The implementation is Unix-specific; other platforms
return an explicit no-go. No cross-architecture or privileged-attacker claim is
made. Exclusion precedence, cursor/recovery, sustained backpressure, and ambient
capture remain separate release gates.

The fixture sentinels are synthetic and not retained; no production path,
account data, credential, browser content, or capture key was used.
