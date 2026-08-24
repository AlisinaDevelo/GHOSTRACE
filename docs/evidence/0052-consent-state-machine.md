# Task 0052 evidence: revocable consent state machine

Status: complete.

Task 0052 adds an explicit, deny-by-default consent ledger. Grants, scope
changes, suspension, revocation, and deletion intent become ordered local
transitions with privacy-bounded receipts. Only an explicit grant can return a
non-active state to active.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0052-CODE-830A639` — `830a639b33db45e0b591a775b33cd6666eedffd8` |
| Implementation pull request | [#189](https://github.com/AlisinaDevelo/GHOSTRACE/pull/189) |
| Protected-main merge | `c16baa19a55be1b64e1cace77ddd720f28c86ce4` |
| Post-merge local pipeline log | `GHOSTRACE-0052-POSTMERGE-LOG-20260824` — SHA-256 `cffe41b35f30fbb85c17b6ed3f36519f38dfd835e450b277fb3a7768e6a1ee7b` |

The raw verification log is retained locally only. It records tool and test
results but does not publish selected roots, event payloads, credentials,
account data, network service data, or private host identifiers.

## Acceptance mapping

1. **Bounded transition receipts.** `ConsentReceipt` records sequence, policy
   identity, policy version, a SHA-256 scope digest, UTC time, bounded actor
   identifier, reason code, transition, and resulting state. Selected roots and
   observations are never serialized into a receipt; the vertical slice checks
   that scope names do not appear in receipt JSON.
2. **Synchronous revocation gate.** `ConsentStateMachine::revoke` commits a
   `revoked` state before any cleanup can run, and `is_capture_allowed` is false
   for inactive, suspended, revoked, and deletion-requested states. Cleanup
   cannot reopen this gate.
3. **Crash/replay safety.** Replay requires contiguous sequence numbers,
   non-decreasing receipt times, matching policy context, and valid transition
   preconditions. Scope changes cannot occur while suspended or revoked, and a
   fabricated non-grant activation is rejected. The complete receipt stream
   replays to the same state and truncation through revocation remains denied.

## Local verification on protected main

Target: the local macOS development environment; no live collector or network
service was enabled. The exact source under test was merge SHA
`c16baa19a55be1b64e1cace77ddd720f28c86ce4`.

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, 1 origin unit test, 24 Rust
  integration tests, clippy, and locked all-target tests.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the enforced denial canary and complete product suite.
- `cargo +1.88.0 doc --no-deps` — pass.
- `cargo +1.88.0 test --locked --release --all-targets --all-features` — pass:
  1 origin unit test, 1 privacy regression, 5 support-matrix tests, and 24
  vertical-slice tests.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` —
  pass.
- `shellcheck scripts/*.sh`, `python3 scripts/roadmap.py check`, `python3
  scripts/fixture-manifest.py check`, `python3 scripts/reproducibility.py
  check`, and `git diff --check` — pass.

Hosted checks on PR #189 were green and served only as protected-branch merge
gates. They are not acceptance evidence for this task; the post-merge local
pipeline above is the evidence record.
