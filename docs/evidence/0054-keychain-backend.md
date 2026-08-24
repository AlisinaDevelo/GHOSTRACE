# Task 0054 evidence: macOS data-protection Keychain backend

Status: complete.

Task 0054 adds an explicit macOS-only provider for the journal wrapping key. It
uses the data-protection generic-password path, disables iCloud synchronization,
requires `WhenUnlockedThisDeviceOnly`, and never creates or recovers a key during
an ordinary read. Keychain failures are reduced to a bounded operation/status
message; service names, account names, access-group values, and key material are
not included in diagnostics.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0054-CODE-0E3423F` — `0e3423f466748846c96aa217424332ca882dcd7c` |
| Implementation pull request | [#193](https://github.com/AlisinaDevelo/GHOSTRACE/pull/193) |
| Protected-main merge | `1137d7e79e8ef04f09d76ef9c347fdbacd526ab1` |
| Post-merge local pipeline log | `GHOSTRACE-0054-POSTMERGE-LOG-20260824` — SHA-256 `fc296e9b246773cc06b1f1a7d214ada508996b1cd75700f1d9a8dd1dd2c712a9` |

The raw verification log is retained locally only. It records tool and test
results but does not publish key material, Keychain identities, credentials,
account data, network service data, or private host identifiers.

## Acceptance mapping

1. **Data-protection and synchronization controls.** `MacOsKeychainProvider`
   configures `use_protected_keychain`, `kSecUseDataProtectionKeychain`, and
   `set_access_synchronized(Some(false))` for exact generic-password searches,
   provisioning, and deletion. Provisioning additionally requires
   `AccessibleWhenUnlockedThisDeviceOnly` and refuses to replace an existing item.
   The dependency is macOS-only and the checked-in lockfile digest is updated in
   `toolchain/manifest.json`.
2. **Identity, entitlement, and session constraints.** Service, account, and
   optional access-group identities are bounded and validated. The default
   service/account are `com.alisinadevelo.ghostrace.journal` and
   `journal-wrapping-key-v1`; an access group is only supplied when a signed
   helper's entitlement matches. `docs/ARCHITECTURE.md`, `docs/PLATFORM.md`, and
   `docs/PRIVACY.md` document the bundle, access-group, login-session, and
   unsigned CLI constraints. The macOS vertical test exercises the provider's
   missing-item and inaccessible-item refusal behavior. On this unsigned local
   helper, Security.framework does not permit the data-protection round trip;
   the test therefore asserts the bounded redacted refusal and returns. No
   legacy-keychain fallback is used or claimed.
3. **Fail-closed item handling.** Reads require exactly one non-synchronizable
   item. Zero items return a missing error, multiple matches return a duplicate
   error, and material other than exactly 32 non-zero bytes returns a malformed
   error. Security.framework errors are mapped to a static operation plus status
   code. Unit tests cover malformed material; the vertical test covers missing
   and inaccessible items and, when a signed Keychain context is available,
   duplicate provisioning and cleanup. The implementation keeps those branches
   fail-closed even when the unsigned local helper cannot reach them.

## Local verification on protected main

Target: the local macOS ARM64 development environment; no live collector or
network service was enabled. The exact source under test was merge SHA
`1137d7e79e8ef04f09d76ef9c347fdbacd526ab1`.

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `cargo +1.88.0 test --locked --all-targets --all-features` — pass: 2 unit
  tests, 1 privacy regression, 5 support-matrix tests, and 26 vertical-slice
  tests (including the macOS Keychain integration test).
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` — pass.
- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, roadmap, fixture manifest,
  clippy, and locked all-target tests.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the enforced denial canary, privacy regression, and complete product suite.
- `cargo +1.88.0 test --locked --release --all-targets --all-features` — pass:
  2 unit tests, 1 privacy regression, 5 support-matrix tests, and 26 vertical
  tests.
- `cargo +1.88.0 doc --locked --no-deps` — pass.
- `shellcheck scripts/*.sh`, `python3 scripts/roadmap.py check`,
  `python3 scripts/fixture-manifest.py check`, `python3
  scripts/reproducibility.py check`, and `git diff --check` — pass.

Hosted checks on PR #193 were green and served only as protected-branch merge
gates. They are not acceptance evidence for this task; the post-merge local
pipeline above is the evidence record.
