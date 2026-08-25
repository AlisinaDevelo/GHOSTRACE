# Task 0008 evidence: Keychain-backed DEK and AEAD envelopes

Status: review candidate. The production implementation is already on protected
`main`; this review adds a focused regression suite and records target-device
receipts. A protected-main rerun and final issue closure remain gated on merge of
this review.

## Contract and implementation

The encrypted journal boundary was implemented and reviewed in
[PR #219](https://github.com/AlisinaDevelo/GHOSTRACE/pull/219),
[PR #223](https://github.com/AlisinaDevelo/GHOSTRACE/pull/223), and
[PR #224](https://github.com/AlisinaDevelo/GHOSTRACE/pull/224). The protected
implementation writes a bounded `GRCE` XChaCha20-Poly1305 envelope before the
SQLite event insert, authenticates event metadata as associated data, resolves
the DEK only through a `KeyProvider`, and refuses missing or malformed Keychain
material. The normal macOS provider uses a non-synchronizable,
`WhenUnlockedThisDeviceOnly` protected generic-password item; it never creates a
key during a read and has no plaintext or environment-variable fallback.

`tests/keychain_aead.rs` adds three executable checks:

1. A missing provider key fails the transaction closed, leaves zero events and
   diagnostics, and does not echo fixture or seed material.
2. A fault at the named event-insert boundary proves key access/encryption is
   reached first; the successful path stores a `GRCE` ciphertext and round-trips
   the payload without retaining the fixture sentinel.
3. Public envelope metadata has no key fields, and the macOS provider's debug
   representation redacts its service and account identities.

The existing vertical-slice, lifecycle, rotation, privacy, and locked-session
tests cover authenticated metadata, deterministic providers, key rotation and
destruction, Keychain provisioning, and the bounded lock/unlock writer gap.

## Acceptance mapping

1. **Encrypted before SQLite insertion.** `insert_events` serializes the payload,
   authenticates associated event metadata, encrypts through the provider, and
   only then executes the event `INSERT`. The `EventBeforeInsert` fault test in
   `tests/keychain_aead.rs` observes a key access before that boundary; the
   successful test requires the stored bytes to begin with `GRCE` and rejects the
   fixture sentinel.
2. **Missing keys fail closed.** `missing_key_fails_closed_before_sqlite_insertion`
   expects `CryptoError::KeyProvider`, an empty journal, and no diagnostics after
   the failed transaction. The default writer policy remains reject; the explicit
   alternative is the bounded `KeyUnavailable` gap tested in `tests/writer.rs`.
3. **No key leakage.** `CiphertextEnvelope` and lifecycle receipts expose only
   schema, algorithm, generation, nonce/ciphertext, counts, and phase. The focused
   test rejects key fields and the macOS provider's `Debug` output contains only
   redaction markers. `tests/privacy_regression.rs` covers untrusted fixture
   values across ingestion, explanation, export, and CLI errors.
4. **Keychain and deterministic providers.** `DeterministicKeyProvider` drives
   offline, reproducible tests; `MacOsKeychainProvider` uses the protected
   Keychain path in production and the isolated explicit-keychain path only in the
   authorized device lifecycle harness. The matrix records observed
   login/unlocked and isolated lock/unlock transitions and explicit no-go rows for
   interactive transitions that were not safely exercised.

## Target-device receipts

All receipts below were run on 2026-08-25 from source commit `a954aab` on a
MacBookPro17,1 (Apple M1, arm64), macOS 26.6.2 build 25G83, Darwin 25.6.0, with
Rust/Cargo 1.88.0 and no network dependency during the offline lanes.

| Lane | Result and retained receipt |
| --- | --- |
| Focused AEAD regression | Pass; `/tmp/ghostrace-0008-source-keychain-aead.log`; SHA-256 `b53ac874c95a10597fd1f8370053dfb583a3fec3ee80e6a9dc00cef0a4c6319a` |
| Debug all-target/all-feature suite | Pass; `/tmp/ghostrace-0008-source-debug.log`; SHA-256 `3deaeb562cf6dfeb21f15b0b73985f5fd4aad3b0e8f1ae230c670676fe9d1021` |
| Clippy with warnings denied | Pass; `/tmp/ghostrace-0008-source-clippy.log`; SHA-256 `10905d84ac09298f3772c2dbb4a3d4d601261717dab64b732a67e2f65635eb5f` |
| Reproducibility/static pipe | Pass; 40 Python tests plus pinned inputs, schema, deterministic demo/export, capture refusal, Rust suite, and clippy; `/tmp/ghostrace-0008-source-repro.log`; SHA-256 `dee0609b9c71620b3680e2ca02443d6917673fc48e5ff57120cd9ba40bc648ed` |
| Explicit network-denial pipe | Pass; canary, privacy focus, and complete suite under macOS `sandbox-exec`; `/tmp/ghostrace-0008-source-offline.log`; SHA-256 `62416bb6714001c9740b0c895f9b7d7175cba7d01cd91de65cfa12d930b95056` |
| Release all-target/all-feature suite | Pass; `/tmp/ghostrace-0008-source-release.log`; SHA-256 `0377ae7b3da5b6938e63cc2fa699301679492c0112fb29d2b92a3015ae00e775` |
| Rust documentation | Pass; `/tmp/ghostrace-0008-source-doc.log`; SHA-256 `4b2ea224c8b640f69ea430e08205ebb85c6fbde2dd1b33e4cfe27b736a3062d2` |
| Shell/action lint | Pass; `/tmp/ghostrace-0008-source-shellcheck.log` and `/tmp/ghostrace-0008-source-actionlint.log` (both empty-success receipts) |
| Keychain lifecycle probe | Pass; `/tmp/ghostrace-0008-source-keychain-lifecycle.log`; SHA-256 `b2d23a5010a0887f9b2fcc57741262c9adf90ed18fbbd797cc835fa95c56a39a` |
| Lifecycle matrix | Pass; `/tmp/ghostrace-0008-source-key-availability-matrix.json`; SHA-256 `7550269db06c2af06876bc4a0cb28e349ade859bb436e39f76ac0128ec936119` |

The lifecycle matrix's observed rows are limited to the current unlocked
session and an isolated Keychain lock/unlock. Sleep, wake, fast-user-switch,
logout, and launchd restart remain explicit `no-go`/`not-exercised` rows; hosted
checks are not used as a substitute for those device transitions.

## Merge gate

After this review is merged, rerun the focused AEAD test, complete debug and
offline lanes, and the release lane at the exact protected-main merge SHA. Add
those post-merge receipt hashes here and only then change task 0008 to `done`,
close issue #12, and publish the zero-delta roadmap result.
