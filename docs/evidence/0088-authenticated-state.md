# Task 0088: authenticated sequence, cursor, policy, and diagnostic state

Status: complete on protected `main`.

This receipt covers local keyed integrity for mutable journal state. It does not
claim remote attestation, collection-origin proof, completeness, or legal chain
of custody.

## Delivery

- Issue: [#92](https://github.com/AlisinaDevelo/GHOSTRACE/issues/92)
- Implementation PR: [#308](https://github.com/AlisinaDevelo/GHOSTRACE/pull/308)
- Implementation commits before squash: `1422b00`, `323879a`
- Protected-main merge: `ee25edd185dce500a4df06856b8b2ecbba67ee3d`
- Reproduction date: 2026-08-26 UTC

The journal now stores a versioned keyed anchor whose canonical,
length-delimited bytes bind event order and identity set, event metadata and
ciphertext, recovery cursors, policy history, diagnostics, key generation,
chain boundaries, and explicit retention deletion markers. `authenticated-check`
returns bounded anomaly classes and states the local-key-only authenticity
limit. Event, cursor, policy, diagnostic, and retention transactions refresh
the anchor before commit; a deleted anchor after bootstrap is never silently
reseeded.

## Acceptance evidence

| Evidence | Acceptance criterion | Result |
|---|---|---|
| E-0088-01 | Canonical bytes, domain separation, chain boundaries, and deletion semantics are defined. | `src/authenticated.rs` defines schema version 1, domain `ghostrace:authenticated-journal-state:v1`, length-delimited canonical fields, keyed chain start/head MACs, key-generation/epoch boundaries, and digest-only deletion markers. Architecture, privacy, and evaluation documentation describe the contract. |
| E-0088-02 | Edits, insertion, deletion, reorder, truncation, cursor rollback, and policy substitution are detected. | `tests/authenticated_state.rs` passed six tests. The negative matrix mutates event content/order/count, cursor state, policy history, diagnostics, and the anchor; reports contain bounded anomaly enums and fail-closed writes. |
| E-0088-03 | Verification does not claim origin authenticity beyond the local key and threat model. | `AuthenticatedStateReport::origin_authenticity_limit()` and the CLI JSON set `local_key_only: true`; documentation explicitly disclaims origin attestation and legal chain of custody. The anchor contains no key material, paths, plaintext, or retained event identifiers. |
| E-0088-04 | Retention deletion is an authenticated chain boundary. | The focused retention test passed; the marker binds plan/candidate digests, snapshot boundary, and counts, advances `chain_epoch`, and retains no event identifiers. |
| E-0088-05 | Missing anchors are observable and never silently reseeded. | The anchor-deletion/reopen test passed; `authenticated-check` reports `anchor_missing` and subsequent file-backed writes fail closed. |

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 (25G83)
MacBookPro17,1, Apple M1, 8 GB / arm64
rustc 1.88.0 (LLVM 20.1.5), host aarch64-apple-darwin
Python 3.9.6
```

## Merged-main verification

Every receipt below ran from `ee25edd185dce500a4df06856b8b2ecbba67ee3d` in the
isolated worktree. Digests are SHA-256 of retained local logs.

| Receipt | Command/result | Log SHA-256 |
|---|---|---|
| E-0088-06 | `scripts/reproducibility-test.sh` — exit 0; pinned inputs, rustfmt, deterministic demo/export, retention, residue, integrity, authenticated JSON determinism, capture refusal, roadmap validators, 46 Python tests, Clippy, all Rust targets, and the native benchmark all passed. | `5dc2b7983bad949eb6f8ab2e173424cd1a6a56f31eac3a35c75795b9cd5d3da4` |
| E-0088-07 | `cargo +1.88.0 check --locked` — exit 0. | `b286eb4801e2613490b11a6e8354de2d4279952259e4ced85e9233766b2a75df` |
| E-0088-08 | `cargo +1.88.0 fmt --all -- --check` — exit 0. | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| E-0088-09 | `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` — exit 0. | `49e15f50b8c19ad7d0bef5f96d05cce9fc7fef8d0beb89886a138eadbb7a5f6f` |
| E-0088-10 | `cargo +1.88.0 test --locked --test authenticated_state -- --nocapture` — 6/6 passed, including happy, tamper, deletion, CLI, and fail-closed paths. | `fc38c9ff2fa749e900e6dff2a9d76a3d53b38cc8926366fd6ef1e9db9bd51462` |
| E-0088-11 | `cargo +1.88.0 test --locked --test keychain_aead -- --nocapture` — 3/3 passed; key-access boundary remains intact. | `e00d9137fc723e4c558d7ee679dfd4e8195b00b69557e66d7688f012bc4214ec` |
| E-0088-12 | `cargo +1.88.0 test --locked --test migrations -- --nocapture` — 7/7 passed; schema version 5 and migration ledger remain deterministic. | `08d82325aa71233eb859cd74b22c51c2d6427895a6d0423281fde424a6238ca8` |
| E-0088-13 | `CARGO_NET_OFFLINE=true cargo +1.88.0 test --locked --all-targets --all-features -- --skip native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` — exit 0; all selected targets passed without network. | `8eaef32d78d276b8abf4616bf6da11ca51f2deb02addef2570749c79dea6423e` |
| E-0088-14 | `cargo +1.88.0 build --locked --release` — exit 0. | `1b6842e037ebf51144d511e5d40364884fd67642928c610b85d382bd1842c345` |
| E-0088-15 | `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` — exit 0. | `c63eddc2784bd6b0d9ee5d66c7df363f897e69914e2b56742febac8852082ac0` |
| E-0088-16 | `cargo +1.88.0 test --locked --all-targets --all-features --release` — exit 0; all release targets, including the native benchmark, passed. | `3901baf0b9b1af7772a2a8ae498bf521d136a67cf5b9a63fd1ec383717fe5cda` |

Protected CI for PR #308 passed both duplicate workflow runs: audit,
clippy, deny, dependency review, roadmap, rustfmt, Linux MSRV, Linux stable,
and macOS stable. The macOS job runs the complete deterministic suite while
excluding only the separately authorized resource-bound native benchmark; the
merged local reproducibility receipt above ran that native benchmark and
passed. Earlier baseline/feature attempts exceeded its 30-second scenario
budget, so that device lane remains performance-sensitive and must retain a
receipt rather than be treated as a universal CI guarantee.

## Scope and limitations

- The key authenticates the local journal state for the configured key
  generation. It is not a remote signature, origin attestation, completeness
  proof, or legal chain of custody.
- Reports are bounded and path-free. They expose counts, digests, key/epoch
  metadata, and anomaly classes, never key bytes, payload plaintext, paths, or
  event identifiers.
- A file-backed journal refuses mutation when the anchor is missing, invalid,
  or the configured key is unavailable. In-memory journals bootstrap on their
  first authenticated transaction because no other process can mutate them.
- Retention authentication records deletion digests and counts only; it does
  not promise SQLite free-page/WAL erasure, compaction, backup removal, or
  external-copy destruction.
