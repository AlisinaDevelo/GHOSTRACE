# Task 0023 evidence: tamper-evident event chain and verifier

Status: implementation, review, merge, and protected-main device verification
complete. Implementation PR [#312](https://github.com/AlisinaDevelo/GHOSTRACE/pull/312)
merged to protected `main` at
`98256981b0ba3faf47c09bf74570e577c77c3738`. The public issue is closed after
the implementation and evidence are independently verified.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0023-01 | The verifier detects payload edits, deletion, reorder, and replay. | `authenticated-check` continues to authenticate the canonical event order, set, and content digests and now reports `event_replayed` when two rows match on every event field except ingest sequence and event identity. `tests/authenticated_state.rs::edits_insertions_deletions_reorder_and_truncation_are_detected` covers edits, insertion/deletion, reorder, and truncation; `::replayed_event_with_a_new_identity_is_detected` copies a ciphertext row under a new UUID and verifies the replay anomaly. The merged release test ran all 8 authenticated-state tests successfully. Replay grouping is read-only verifier work, not an authenticated-write hot-path allocation. |
| E-0023-02 | Anchor handling survives key rotation. | `KeyRing::activate_generation` exposes the safe active-generation switch while retaining prior generations. `tests/authenticated_state.rs::key_rotation_advances_the_authenticated_chain_boundary` verifies the old generation before the first post-rotation write, then verifies generation 2 and `chain_epoch == 1` after the boundary write. Older generations remain available until state and ciphertext verification is complete. |
| E-0023-03 | Documentation makes no legal chain-of-custody claim. | `README.md` and `docs/ARCHITECTURE.md` describe replay detection, key-rotation boundaries, local-key-only validity, and the explicit limitation that this is not origin attestation or a legal chain-of-custody claim. |

## Delivery

- Issue: [#27](https://github.com/AlisinaDevelo/GHOSTRACE/issues/27)
- Implementation PR: [#312](https://github.com/AlisinaDevelo/GHOSTRACE/pull/312)
- Implementation commits before squash: `a4ef5a3`, `0fe0028`
- Protected-main merge: `98256981b0ba3faf47c09bf74570e577c77c3738`
- Evidence reproduction date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 (25G83)
MacBookPro17,1 / Apple M1 / arm64 / 8 GB
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
merged source revision: 98256981b0ba3faf47c09bf74570e577c77c3738
```

## Merged-main device verification

Every command in this section ran after the implementation merge from the
exact protected-main SHA above. Hosted checks are corroboration; the retained
device logs are the acceptance evidence.

### Tamper, replay, rotation, recovery, and privacy

- `cargo +1.88.0 test --locked --release --test authenticated_state -- --nocapture` exited `0`; 8/8 tests passed, including payload edit/deletion/reorder/truncation, new-identity replay, key rotation, CLI JSON failure behavior, cursor/policy/diagnostic substitution, and anchor deletion.
- The deterministic pipe ran `scripts/reproducibility-test.sh` and exited `0`: pinned-input checks, schema/deterministic CLI/export/retention/integrity/authenticated-state flows, signed-checkpoint/repair demo, capture refusal, 46 Python tests, Clippy, and all deterministic Rust targets (the separately gated native resource test is excluded by the script).
- A sandbox with `(deny network*)`, `CARGO_NET_OFFLINE=true`, and the checked-in enforcement variables ran the network-denial canary, privacy regression, and all offline Rust targets while skipping only the native resource test; it exited `0`.
- `cargo +1.88.0 build --locked --release`, `cargo +1.88.0 fmt --all -- --check`, and `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` all exited `0`.

### Native resource lane and explicit no-go

The merged-SHA native resource command was attempted on the named Mac:

```text
GHOSTRACE_BENCHMARK_REVISION=98256981b0ba3faf47c09bf74570e577c77c3738 \
  cargo +1.88.0 test --locked --test filesystem_benchmark -- --nocapture \
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt
```

It exited `101` after `127.06s` because an existing synthetic filesystem
scenario exceeded the test's 30-second per-scenario bound; no resource receipt
was emitted. This is an explicit resource-lane no-go, not a pass claim. The
unchanged merged-main baseline `b7e161cb122a3d5d1b79a9576ba5f6768b222bf3`
failed the same assertion after `139.42s`, so the failure is environmental
FSEvents timing rather than a 0023 regression. The smaller native safe-storm
lifecycle/resource test inside the deterministic suite passed. The bound is
retained; it is not weakened to manufacture a receipt.

## Hosted review and protected merge

PR #312 was pushed from `feature/tamper-evident-verifier`, reviewed through the
protected-branch workflow, and squash-merged only after both duplicate CI runs
were green: Linux stable, Linux MSRV, macOS stable, rustfmt, Clippy, roadmap,
Cargo policy/deny, Rust advisories, dependency review, and offline fixture
lanes. Required status checks were not bypassed.

## Retained artifacts

| Artifact | Result | SHA-256 |
|---|---|---|
| `/tmp/ghostrace-0023-merged-repro.log` | deterministic merged-main pipe, exit 0 | `6a312ce262357c99e067a726640dda65e1d33ddeb07094fee00aac839af29b43` |
| `/tmp/ghostrace-0023-merged-fmt.log` | format check, exit 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/tmp/ghostrace-0023-merged-clippy.log` | Clippy `-D warnings`, exit 0 | `a178c7902ac24dda0e2c0cd4cf0e47a2a6976ef30481cb4015f2f71620380d8b` |
| `/tmp/ghostrace-0023-merged-release-build.log` | release build, exit 0 | `1ff8bbcb4151dc887e69b78a29bc241d5062cba4757aa5f4639926ae712a8181` |
| `/tmp/ghostrace-0023-merged-release-auth.log` | release authenticated-state tests, 8/8, exit 0 | `08ec946667cc4653b1307bce9266877425e3d01291b114806cb4ed477290c48a` |
| `/tmp/ghostrace-0023-merged-doc.log` | rustdoc with warnings denied, exit 0 | `fcec60bd152a81edc175363d6e74fd29bfa095d46c1e622132ccbfcebbf2f90e` |
| `/tmp/ghostrace-0023-merged-offline-non-native.log` | sandbox canary/privacy/all non-native targets, exit 0 | `4718a84df3e57b4773fd105a7357963e2666000ffc605abbc62339b88c162f9d` |
| `/tmp/ghostrace-0023-merged-native.log` | merged native resource no-go, exit 101 | `a15e54225167ea82204bb8302cef83af2dbc75f6ab1380ee66b9f21eabcf53f7` |
| `/tmp/ghostrace-0023-native-baseline-control.log` | unchanged-main native control, exit 101 | `a187fa5810c25f3796d6037061071deb057cfbec99e6847fd1d85f405dd36bb7` |

## Privacy, failure, and scope boundaries

- Replay and authentication prove possession of the configured local journal
  key and consistency of the bounded local state; they do not prove event
  origin, complete collection, operator identity, or legal chain of custody.
- Replay matching excludes only ingest sequence and event identity. Legitimate
  identical observations can therefore be reported as a copied-row replay;
  the report is a bounded anomaly signal, not a claim that identical real-world
  observations are impossible.
- Key bytes, payload plaintext, event IDs, paths, and external recovery secrets
  are not serialized into the authenticated report or public documentation.
- Existing key generations are retained until all states and ciphertexts that
  name them are independently verified; retirement remains an explicit
  destruction boundary.
- The native resource no-go and the existing guarded Keychain lifecycle test
  remain visible limitations. They are not converted into successful evidence
  by skipping or weakening their guards.
