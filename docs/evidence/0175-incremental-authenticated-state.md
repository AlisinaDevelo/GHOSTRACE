# Task 0175 evidence: incremental authenticated-state verification

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR
[#425](https://github.com/AlisinaDevelo/GHOSTRACE/pull/425) was squash-merged to
protected `main` at `a55e686`.

The deliverable is the v2 authenticated anchor, per-row commitments, and keyed
operation ledger (migration 0006, `src/authenticated.rs`, `src/journal.rs`).

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0175-01 | A write verifies and advances the anchor from the previous anchor and the rows it adds, not by recomputing digests over every stored event. | Steady-state writes authenticate the prior anchor and advance only the changed rows through the keyed operation ledger. At 100,000 events the 64 measured hot writes peaked at 9.014 ms (`auth-100k.log`). The pre-incremental baseline recorded by the implementing session on 2026-10-03 (`backlog-20261003.V4wTWs/auth-ce07dab-100k-200ms-red-20261003.log`, SHA-256 `bc9dbda7b4b3b0300aaf0e4d88563ecbf375ac662f2b4c4223b9c73e499c17a3`, not rerun here) peaked at 5,530 ms. |
| E-0175-02 | The full recomputation remains available and runs in `authenticated-check`, at startup after an unclean shutdown, and when another process's commit is detected. | Every reopened writer performs a full scan before its first mutation (16,120 ms at 100,000 events, one key read), a foreign commit detected through the data_version fence forces full verification, and the final full check took 8,169 ms and was valid (`auth-100k.log`). Tamper, trigger, ledger-rewrite, rollback, retention, rotation, and v1-promotion cases are in `tests/incremental_authentication_adversarial.rs` and `tests/authenticated_state.rs` (`all-targets.log`). |
| E-0175-03 | Write-lock hold time stays within a documented bound at 100,000 events on the reference device, and two concurrent writers never time out under the default busy timeout. | Predeclared bound 200 ms; maximum whole-write time 9.014 ms, which includes the lock interval. Two connections and two separate processes each completed 32 writes with the default 250 ms busy timeout and no timeout or refusal (`auth-100k.log`). |

The key is read once per write transaction: `key_reads_per_hot_write=1`, and
`tests/keychain_aead.rs` pins exactly one read on the insert, batch, bootstrap,
preflight, and file-backed hot paths.

## Delivery

- Issue: [#403](https://github.com/AlisinaDevelo/GHOSTRACE/issues/403)
- Implementation PR: [#425](https://github.com/AlisinaDevelo/GHOSTRACE/pull/425), implemented by Codex and reviewed before push (an earlier revision read the key four times per write and was sent back).
- Protected-main merge: `a55e686`
- Verification date: 2026-10-04 UTC

## Device and toolchain

```text
Darwin 25.6.0
26.6.2
MacBookPro17,1
arm64
rustc 1.88.0 (6b00bc388 2025-06-23)
host: aarch64-apple-darwin
verified source revision: a55e686a95c6dd7ab91e5f453d7a0de965b64d30
```

## Merged-main device verification

Every command ran from protected `main` at `a55e686` with a private target directory.

- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited `0`: 459 passed, 0 failed.
- `GHOSTRACE_AUTH_HOT_WRITE_BOUND_MS=200 cargo +1.88.0 test --locked --all-features --test authenticated_state one_hundred_thousand_events_and_two_default_timeout_writers -- --ignored --exact --nocapture` exited `0` (unoptimized test profile).

## Retained artifacts

Logs are kept in `~/Developer/dev/ghostrace-evidence`, outside the repository and temporary storage.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-a55e686/device-info.txt` | device/toolchain capture | `286d5a70b5eeb1a165ef15d5095056e45ebe486ca117319e02bb6d3f0f81dafc` | 172 |
| `ghostrace-evidence/merged-a55e686/clippy.log` | Clippy with warnings denied, exit 0 | `af30a45be73f13ef38e64f081cf0b619c705f3eeb293d1c6cbbab380a998c089` | 5169 |
| `ghostrace-evidence/merged-a55e686/all-targets.log` | all targets except native benchmark, 459 passed | `f7d71ab166dc2e7dca8417ef43531f1a095da0d6862628d8cc031cf39664265a` | 61872 |
| `ghostrace-evidence/merged-a55e686/auth-100k.log` | 100,000-event acceptance, exit 0 | `15296b92b83120be6a640a21fe581e7a953a7193c4737a99af913f1c0db6b207` | 1856 |

## Limits

- The startup full scan grows with live rows and operation history (16.1 s at 100,000 events here). It is inside the 30 s reader limit on this device but not bounded on larger journals or slower devices.
- The bound covers steady-state single-event writes, not startup, promotion, retention, rotation, or large batches.
- A local key cannot detect rollback of the whole database to an earlier valid snapshot without an external monotonic witness.
