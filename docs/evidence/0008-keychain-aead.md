# Task 0008 evidence: Keychain-backed DEK and AEAD envelopes

Status: complete on protected `main` at
`fd5213fb2576c40c918df6be549596e0e8f8a568`. The production implementation and
focused regression suite are merged, and the same device matrix was rerun at
that exact SHA.

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

## Protected-main rerun receipts

The merge of [PR #225](https://github.com/AlisinaDevelo/GHOSTRACE/pull/225) was
verified at `fd5213fb2576c40c918df6be549596e0e8f8a568` on the same device before
this task was closed.

| Lane | Result and retained receipt |
| --- | --- |
| Focused AEAD regression | Pass; `/tmp/ghostrace-0008-postmerge-keychain-aead.log`; SHA-256 `2c77624a9d79a27c27ae22d21f4ba13e47cc81614da6d2e1a086587a85b3e178` |
| Debug all-target/all-feature suite | Pass; `/tmp/ghostrace-0008-postmerge-debug.log`; SHA-256 `32534ea8e67c362b65ca86e9d5c9b01ee495f1d5bfc2643b8dac53da1031587e` |
| Clippy with warnings denied | Pass; `/tmp/ghostrace-0008-postmerge-clippy.log`; SHA-256 `f2066ca264b9a8598ecd436c70919e1efe1acea4098ad2a13b5d82d67bc049ec` |
| Reproducibility/static pipe | Pass; 40 Python tests and the complete pinned suite; `/tmp/ghostrace-0008-postmerge-repro.log`; SHA-256 `566a857fd9e93e791271e4ad222a496d6d5e70525b32fd3b7331c5a204ce7283` |
| Explicit network-denial pipe | Pass; `/tmp/ghostrace-0008-postmerge-offline.log`; SHA-256 `4b46ec3deb51d085f76b56462a14b3171116d1a3de39ff4f261f4784a93e8091` |
| Release all-target/all-feature suite | Pass; `/tmp/ghostrace-0008-postmerge-release.log`; SHA-256 `d26880c4963e1eaf6fb560b52327e65d8f52c088293756b60563ebfd53ce559e` |
| Rust documentation | Pass; `/tmp/ghostrace-0008-postmerge-doc.log`; SHA-256 `4b2ea224c8b640f69ea430e08205ebb85c6fbde2dd1b33e4cfe27b736a3062d2` |
| Shell/action lint | Pass; `/tmp/ghostrace-0008-postmerge-shellcheck.log` and `/tmp/ghostrace-0008-postmerge-actionlint.log` (both empty-success receipts) |
| Keychain lifecycle probe | Pass; `/tmp/ghostrace-0008-postmerge-keychain-lifecycle.log`; SHA-256 `a05a3e8f321e649d0b4630ea635730e6891bb09b4b3a3cfd3625f7786a928ed1` |
| Lifecycle matrix | Pass; `/tmp/ghostrace-0008-postmerge-key-availability-matrix.json`; SHA-256 `3945478347e37670094c09069176eb6c03b0e55aeb923fcb908f1200397b6c9a` |

The lifecycle matrix's observed rows are limited to the current unlocked
session and an isolated Keychain lock/unlock. Sleep, wake, fast-user-switch,
logout, and launchd restart remain explicit `no-go`/`not-exercised` rows; hosted
checks are not used as a substitute for those device transitions.

## Closure

Issue #12 can be closed against this evidence. The lifecycle matrix's observed
rows remain limited to the current unlocked session and an isolated Keychain
lock/unlock; sleep, wake, fast-user-switch, logout, and launchd restart remain
explicit `no-go`/`not-exercised` rows. No hosted check is used as a substitute
for those device transitions.
