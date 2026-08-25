# Task 0067 evidence: filesystem-aware path scope

Status: complete on protected `main`. Implementation PR #244 merged at
`37b8e177b64e98138004b7dcbcd6e2abe0afab1b` on 2026-08-25. The receipts below
were produced after that merge on the target Mac; hosted checks were merge gates,
not a substitute for the local replay.

## Contract implemented

`SelectedRoot` canonicalizes an absolute directory and records its private
device/inode identity. `contains_path` rejects relative paths and `..`, resolves
existing components through the operating system, checks component-aware
containment, rejects a different device beneath a matching lexical prefix, and
invalidates a root whose directory has been replaced. The collector uses that
same check before admitting a filesystem event.

No case folding or Unicode normalization is invented. The filesystem decides
whether composed/decomposed or case variants resolve to one canonical identity.
`path_digest` is a scoped SHA-256 domain containing the opaque root ID, root
device/inode, and OS-canonicalized path bytes. It is intentionally not a
cross-volume or cross-root identifier; a missing leaf falls back to its supplied
path bytes within the already-validated root.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Unicode, case-only rename, normalization collision, and mixed-volume coverage | `tests/selected_root_scope.rs` creates composed `café` and decomposed `cafe\\u{301}` names and compares canonical identity/digests; the case-only rename fixture applies the same rule. Unit tests cover a lexical-prefix sibling, parent traversal, a simulated different-device descendant, and replacement of the selected root. The tests deliberately compare the OS result rather than asserting a made-up APFS normalization rule. |
| Filesystem-aware containment rejects lexical escapes | `SelectedRoot::contains_path` resolves the nearest existing component, rejects `ParentDir`, requires the root's current device/inode, requires the candidate device to match, and uses component-aware `Path::starts_with`. The focused collector/scope receipt passes these paths on this device. |
| Digest stability is limited to documented scope | `path_digest` includes the `ghostrace-fsevents-path-digest-v2` domain, opaque root ID, root device/inode, and canonical bytes. The unit test proves two root IDs produce different digests and that the digest does not contain the source path. The integration fixtures assert equality only when the OS canonical paths are equal. |

## Device and retained receipts

Receipts were run on 2026-08-25 from the exact protected-main commit above on a
MacBookPro17,1 (Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0,
Rust/Cargo 1.88.0, target `aarch64-apple-darwin`, and Python 3.9.6.

| Lane | Result and receipt |
| --- | --- |
| Focused protected-main scope suite | Pass: 3 selected-root collector tests and 2 path-scope tests; `/tmp/ghostrace-0067-postmerge-scope.log`; SHA-256 `8ace3f202f6272c1f08227e77f0828faf8d952bbf8dc972bd822028d085d7907` |
| Full reproducibility pipe | Pass: pinned inputs, fixture manifest, rustfmt/schema, deterministic fixture CLI/export, capture refusal, roadmap/evidence checks, 40 Python tests, and all debug Rust targets; `/tmp/ghostrace-0067-postmerge-repro.log`; SHA-256 `6eef9dfeb262102583502bac326703ef25d7901a395d6669763a929515ae31a2` |
| Release all-target/all-feature tests | Pass: 27 library tests and all integration targets; expected locked-Keychain and network-canary tests remained explicitly ignored; `/tmp/ghostrace-0067-postmerge-release.log`; SHA-256 `478e9d46c968f08452634fff49c41ba9bdffde4857e3fca3f852e3e92dd538be` |
| Rustdoc warnings denied | Pass with `RUSTDOCFLAGS='-D warnings'`; `/tmp/ghostrace-0067-postmerge-doc.log`; SHA-256 `12c31f130a12d68ea6f2f9343336da176ea0722894d50998becea6e97e9ed391` |
| Offline/network-denial lane | Pass under `sandbox-exec`: denial canary, privacy regression, and complete offline locked product suite; `/tmp/ghostrace-0067-postmerge-offline.log`; SHA-256 `e49eb81850d502d6924f9e055d233928e68c7a4f1ce9fb2f618638e082b18bd6` |

Before merge, PR #244 also passed the duplicate hosted Linux stable/MSRV and
macOS stable suites, Clippy with `-D warnings`, rustfmt, roadmap, Cargo policy,
dependency review, advisories, and the network-denial fixture lane.

## Limits and next gate

This is a metadata-only scope boundary: the collector does not open or read a
reported path. The resolution is snapshot-based and therefore does not claim
race-resistant symlink/hard-link or validation-to-use protection; those are the
next containment gate. The mixed-volume test models device identity rather than
requiring a second external volume, and no cross-architecture claim is made.

The fixture sentinel is synthetic and is not retained. No production path,
account data, credential, browser content, or capture key was used.
