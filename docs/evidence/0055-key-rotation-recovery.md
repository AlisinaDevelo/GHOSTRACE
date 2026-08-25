# Task 0055 evidence: key rotation, recovery, and destruction

Status: complete for the local encrypted-payload and key-lifecycle boundary. This
gate does not enable live collection, a signed Keychain helper, or a cloud recovery
service.

## Contract and implementation

The implementation is [PR #219](https://github.com/AlisinaDevelo/GHOSTRACE/pull/219),
authored at `5dc76ca7d974e7258f1f4520a4f271916d5e9d77` and squash-merged to protected
`main` at `86319170b0c95e60b4a87fc9613302d3e04e5d0d` on 2026-08-25.

`src/crypto.rs` writes a bounded, authenticated `GRCE` envelope containing the
schema version, algorithm, key generation, nonce, and ciphertext. Key bytes are not
part of the envelope, JSON metadata, diagnostics, or rotation checkpoint. Readers
continue to accept the legacy nonce-plus-ciphertext form while existing rows are
migrated.

`src/key_lifecycle.rs` retains a bounded in-memory key ring and exposes a two-phase
rotation. A new generation is staged, each replacement is decrypted and re-encrypted
and then verified, and only a complete explicit commit retires the prior generation.
The checkpoint is resumable and contains only generations, counts, and phase. Lost-key,
compromise, and user-reset operations require an explicit scope-matching confirmation
and return a receipt that lists destroyed generations and states exactly when their
ciphertext is unrecoverable. No cloud recovery secret or plaintext queue is introduced.

The strict `schemas/key-lifecycle-v1.json` contract and
`tests/key_lifecycle.rs` cover envelope metadata, legacy reads, tamper/authentication
boundaries, staged rotation, checkpoint round trips, commit retirement, confirmation
failures, reset/destruction receipts, unknown-field rejection, and absence of key
material from public values.

## Acceptance mapping

1. **Envelope metadata without key material.** `CiphertextEnvelope` encodes the
   algorithm and positive generation in the binary header; `KeyMetadata` and the
   strict schema expose no key field. The test checks the round trip and validates the
   serialized envelope against the schema.
2. **Resumable rotation and verified retirement.** `KeyRotation::begin` stages the
   next generation, `reencrypt` verifies each replacement and advances a key-free
   checkpoint, and `resume` continues from that checkpoint. The old envelope remains
   readable before commit; after commit the old generation is absent and the new
   envelopes decrypt successfully.
3. **Explicit loss/reset semantics.** Destruction rejects unconfirmed or
   mismatched-scope requests and refuses to destroy the active generation. A confirmed
   generation destroy or all-generation reset returns a bounded receipt with the exact
   destroyed generations, reason, and `data_unrecoverable` outcome.

## Target-device verification

All receipts below were run on 2026-08-25 from the protected merge SHA above on a
MacBook Pro 17,1 (Apple M1, 8 GB, arm64), macOS 26.6.2 (25G83), Darwin 25.6.0, with
Rust/Cargo 1.88.0.

| Lane | Result and local receipt |
| --- | --- |
| Pre-merge optimized all-target/all-feature suite | Pass; `/private/tmp/ghostrace-0055-release-all-v2.log`; SHA-256 `1ab0c0c424c7dab6a974886a629e7341f688d749a6a4198608ce6d6605fa3e70` |
| Pre-merge macOS sandbox-exec offline canary, privacy focus, and complete suite | Pass; `/private/tmp/ghostrace-0055-sandbox-v1.log`; SHA-256 `182f1ddb816ce4bd68771ba1eb4f576d729414fae537d52ce54deb4de7655ee7` |
| Pre-merge reproducibility/static pipe | Pass; 38 Python tests, locked Clippy with warnings denied, ShellCheck, actionlint, docs, roadmap, and source-only network scan; `/private/tmp/ghostrace-0055-static-v2.log`; SHA-256 `1105b7b130135c76f7a78835889be868fc2531e307e6fd0328a69c71d825062f` |
| Post-merge optimized all-target/all-feature suite | Pass; `/private/tmp/ghostrace-0055-postmerge-release-v1.log`; SHA-256 `2e29b46250b16f2ea9707096a5cd9578724943792e16a0966073b0ddee7655d6` |
| Post-merge macOS sandbox-exec offline canary, privacy focus, and complete suite | Pass; `/private/tmp/ghostrace-0055-postmerge-sandbox-v1.log`; SHA-256 `cceabeb9b0cad61de373d4ac15bacddef51ca7d814f12523362571964420c633` |
| Post-merge reproducibility/static pipe | Pass; 38 Python tests, locked Clippy with warnings denied, ShellCheck, actionlint, docs, roadmap, and source-only network scan; `/private/tmp/ghostrace-0055-postmerge-static-v1.log`; SHA-256 `d4ae63545dc6c35bcf82bd007b23b10b7dcbc476dffe491962e1910f2cb68ed7` |

The first pre-merge optimized attempt is retained separately as
`/private/tmp/ghostrace-0055-release-all-v1-failed.log`: release builds exposed that
the existing migration crash hook was compiled out, so the child did not abort. The
environment-gated hook was made profile-independent, the focused release recovery
test passed for all three migrations, and the complete v2 receipt above passed. A
separate debug focused attempt also hit device `ENOSPC` before compilation; it is not
counted as evidence.

Hosted protected checks on PR #219 were all green (audit, Clippy, dependency review,
Cargo policy, offline fixture, roadmap, rustfmt, Linux MSRV/stable, and macOS stable).
They are merge gates, not a substitute for the post-merge device receipts above.

## Limitations

- The key ring is an explicit local primitive; a production signed-helper Keychain
  integration and locked-session behavior remain separate gates.
- Rotation verifies supplied records but does not yet provide a database-wide
  migration command or live collector integration.
- Destroyed generations cannot be recovered from this component; external exports or
  filesystem snapshots remain outside its control and are called out by the privacy
  model.
