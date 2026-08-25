# Task 0056 evidence: locked-session and background key behavior

Status: complete for the bounded writer/key-provider contract on protected
`main`. The matrix deliberately records transitions that were not safe to
automate as explicit no-go results; it does not turn those rows into positive
support claims.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `069cde6fed9ac7770d2f8f59c93b3d3e503cbdeb` |
| Implementation pull request | [#223](https://github.com/AlisinaDevelo/GHOSTRACE/pull/223) |
| Protected-main merge | `7aaa5e2ebc944d51908e56be1d87947f78f192d2` |
| Device lifecycle log | `/tmp/ghostrace-0056-keychain-lifecycle-source-final.log` — SHA-256 `24873913bfc572ac18af2183204bf739def842320a71f61efb9db0e9cd9af8ab` |
| Machine-readable matrix | [`0056-key-availability-matrix.json`](0056-key-availability-matrix.json) |
| Source reproducibility pipe | `/tmp/ghostrace-0056-source-repro-final.log` — SHA-256 `1c4f3014268482d613c50c9e6ae027a06b80f7fbf9fb381329a3718b6986d145` |
| Source network-denial pipe | `/tmp/ghostrace-0056-source-offline.log` — SHA-256 `785a3a5e11d37054bffaa24a37e2b95623198087e207f5482f8d23a1b06752eb` |
| Source release all-target pipe | `/tmp/ghostrace-0056-source-release.log` — SHA-256 `c9eaeedb0ef3f543627d5313eca208616ffe5d34a5ddab60bd3d8fb69c75f400` |
| Post-merge device lifecycle log | `/tmp/ghostrace-0056-keychain-lifecycle-postmerge.log` — SHA-256 `b22a81e2f67b36ef370b10eee919183a6947ce1ce3cf473410ceb458e3b9d9b1` |
| Post-merge reproducibility pipe | `/tmp/ghostrace-0056-postmerge-repro.log` — SHA-256 `ebc9fc56dc98407739aeab670c5e7535c1d8a88f065f7b7dd38802f45764be0d` |
| Post-merge network-denial pipe | `/tmp/ghostrace-0056-postmerge-offline.log` — SHA-256 `63a4705f2d38e20843be3f802dd894d2192fabf09a75f6aa021f0ef6630a8a3f` |
| Post-merge release all-target pipe | `/tmp/ghostrace-0056-postmerge-release.log` — SHA-256 `63a9ad024bd2ec83b6bb9b31534d9af73a0257aa28ec9b772876f3a244f83487` |

The source and post-merge runs used a MacBookPro17,1 (Apple M1, arm64), macOS
26.6.2 build 25G83, with Rust/Cargo 1.88.0 (`aarch64-apple-darwin`). The probe used an isolated
temporary legacy Keychain and restored the user's default/search Keychain list
in an RAII cleanup path. No login-keychain item, user password, key material,
event payload, or private path was written to the repository or evidence.

## Lifecycle matrix

The JSON artifact has one row for each required transition: login/unlocked,
lock, sleep, wake, fast-user-switch, logout, and launchd-restart.

- **Observed:** login/unlocked returned the provisioned key without prompting;
  locking the isolated Keychain made the provider fail closed, caused the
  writer's explicit `KeyUnavailable` gap, and unlocking recovered a committed
  event.
- **Explicit no-go:** screen sleep/wake were not triggered because they suspend
  the active device session; fast-user-switch had no second authorized test
  account; logout would terminate the harness; and no GHOSTRACE launchd helper
  is enabled to restart. Each row records `not-exercised`,
  `interactive-required`, and the reason. Hosted CI is not used as a substitute.

## Acceptance mapping

1. **A macOS matrix records Keychain availability and prompts for every
   transition.** `0056-key-availability-matrix.json` is schema-validated by
   `tests/keychain_matrix.rs`. It records availability, prompt behavior, buffer
   behavior, evidence, and limitation for all seven transitions; it makes no
   positive claim for the five no-go rows.
2. **Buffer only within a bound or emit a gap when the key is unavailable.**
   `WriterConfig::key_unavailable_policy` defaults to `Reject`; explicit
   `EmitGap` returns a source-labelled `WriterGap` with `KeyUnavailable` and
   the admitted batch count. `key_unavailable_rejects_by_default_without_plaintext_or_silent_loss`,
   `key_unavailable_can_emit_a_bounded_gap_without_plaintext_or_silent_loss`,
   `key_unavailable_gap_is_bounded_to_the_admitted_batch`, and the device
   lifecycle test cover the reject, bounded-gap, and recovery paths.
3. **No fallback key, plaintext queue, or silent loss.** The locked provider
   never generates a key; the failed transaction leaves zero journal events,
   the explicit gap is the only loss signal, and outstanding writer memory
   returns to `(0, 0)`. The matrix privacy contract is all false for
   `fallback_key`, `plaintext_queue`, and `silent_loss`, and the full privacy
   regression suite passes.

## Local verification

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `cargo +1.88.0 test --locked --offline --all-targets --all-features` — pass.
- `cargo +1.88.0 clippy --locked --offline --all-targets --all-features -- -D warnings` — pass.
- `cargo +1.88.0 test --locked --offline --release --all-targets --all-features` — pass.
- `cargo +1.88.0 doc --locked --offline --no-deps` — pass.
- `python3 -m unittest discover -s tests -p 'test_*.py'` — pass: 40 tests.
- `scripts/reproducibility-test.sh` — pass, exit 0.
- `scripts/offline-network-test.sh` — pass, exit 0; denial canary enforced.
- `shellcheck scripts/*.sh`, `actionlint`, `python3 scripts/roadmap.py check`,
  `python3 scripts/fixture-manifest.py check`, and `git diff --check` — pass.

The protected-main reproduction above is the acceptance rerun for merge
`7aaa5e2ebc944d51908e56be1d87947f78f192d2`; hosted checks on PR #223 were
additional merge gates, not substitutes for the device receipts.
