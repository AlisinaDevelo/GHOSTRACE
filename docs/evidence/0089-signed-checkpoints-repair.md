# Task 0089 evidence: signed checkpoints and verified-copy repair

Status: implementation, review, merge, and protected-main device verification
complete. Implementation PR [#310](https://github.com/AlisinaDevelo/GHOSTRACE/pull/310)
merged to protected `main` at
`799a1ff3c787bfd64c57d9e3c0d2b1aa3a951978`. The public issue is closed only
after this evidence change is merged and independently verified.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0089-01 | Checkpoints bind chain position, database identity, schema, key generation, policy set, and verification time. | `src/recovery.rs` defines strict schema-v1 `VerificationCheckpoint` fields for checkpointed database identity, journal schema, chain epoch/head, event count/max sequence, key generation, policy digest, integrity digest, and RFC3339 verification time. Canonical length-delimited bytes are signed with a local HMAC-SHA-256 key; the key is never serialized. `tests/recovery.rs::checkpoint_binds_state_and_rejects_tampering_or_mutation` covers signature/head tampering and post-checkpoint mutation. |
| E-0089-02 | Repair operates on a verified copy, emits before-and-after manifests, and records every dropped or reconstructed interval as a gap. | `Journal::repair_verified_copy` checkpoints and verifies the source, copies only the checkpointed database, verifies the copy, refuses empty/overlapping/fixture/overbound intervals and child/cursor-tail orphaning, applies one bounded transaction, appends one repair-origin gap per interval, verifies the after checkpoint, and re-verifies the source. `RepairManifest` reconciles before/after counts and exposes dropped, reconstructed, gap, and interval counts. Reconstruction is not claimed by this MVP (`reconstructed_event_count: 0`); every selected dropped interval is represented by an explicit gap. |
| E-0089-03 | The normal writer refuses a journal with unresolved integrity failures. | `Journal::ensure_authenticated_for_write` performs a bounded integrity/foreign-key check when SQLite `data_version` changes and returns `IntegrityReportInvalid` before a normal write on failure. `tests/recovery.rs::integrity_failure_stops_normal_writer` tampers with a foreign-key row and verifies the next ingest is refused. |
| E-0089-04 | The public command surface is reproducible and privacy-safe. | `checkpoint`, bounded `repair`, and `recovery-demo` are documented in `README.md` and `docs/ARCHITECTURE.md`. `scripts/recovery-demo.sh` asserts a verified copy, count reconciliation, one dropped event/one gap, and no temporary path or seed leakage. |

## Delivery

- Issue: [#93](https://github.com/AlisinaDevelo/GHOSTRACE/issues/93)
- Implementation PR: [#310](https://github.com/AlisinaDevelo/GHOSTRACE/pull/310)
- Implementation commit before squash: `86f7524`
- Protected-main merge: `799a1ff3c787bfd64c57d9e3c0d2b1aa3a951978`
- Evidence reproduction date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 (25G83)
MacBookPro17,1 / Apple M1 / arm64 / 8 GB
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
Python 3.9.6
```

## Merged-main device receipts

All receipts in this section were generated after the implementation merge,
from protected `main` at `799a1ff3c787bfd64c57d9e3c0d2b1aa3a951978`.

### Recovery MVP

`scripts/recovery-demo.sh` exited `0`. Its path-free manifest has
`verified_copy: true`, before/after event counts `2 → 2`,
`dropped_event_count: 1`, `reconstructed_event_count: 0`, and
`gap_event_count: 1`. The retained JSON output is SHA-256
`2cf4d03ccc4a7f9507994261b5d21333433ed8df832125f686a5ad3b220a66f7`.

### Native filesystem/resource lane

The clean-source direct command below ran the exact native test with the
merged SHA supplied as `GHOSTRACE_BENCHMARK_REVISION`; it exited `0` after 24
scenario runs (three repetitions of eight synthetic workloads). The parsed
path/content-free report is retained locally.

```text
GHOSTRACE_BENCHMARK_REVISION=799a1ff3c787bfd64c57d9e3c0d2b1aa3a951978 \
  cargo +1.88.0 test --locked --test filesystem_benchmark \
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt \
  -- --exact --nocapture
```

| Measurement | Value |
|---|---:|
| Scenario runs | 24 |
| Coverage | 373 contextual / 789 direct / 0 inferred / 0 unknown |
| Duplicate rate | 0.0 |
| Gap rate | 0.5905096660808435 |
| Failure counts | `cursor_regression: 1` (retained as a bounded gap) |
| Latency p50 / p95 / p99 | 1717.090542 / 12829.451417 / 22241.169459 ms |
| CPU user / system | 116008.908 / 1029.696 ms |
| Peak RSS | 15679488 bytes |
| Disk growth | 5872224 bytes |
| Energy | `0` (explicit no-go: no measurable privileged power delta) |

Device context in the receipt is `MacBookPro17,1`, macOS `26.6.2`, arm64,
rustc `1.88.0 (6b00bc388 2025-06-23)`, and source revision
`799a1ff3c787bfd64c57d9e3c0d2b1aa3a951978`.

| Artifact | SHA-256 |
|---|---|
| Native command log `/tmp/ghostrace-0089-merged-native-clean-final.log` | `1171662e685e123b39d57a0014fa29a875cdd3a66d7d063df6ddea07db5cb246` |
| Parsed report `/tmp/ghostrace-0089-merged-native-clean-final-report.json` | `215dc98626bd605bd951d0b78de6a16b5a176e59145ee1a275ba12bd63f871dd` |

The resource guard is intentionally retained. Earlier standalone attempts
hit the existing 30-second per-scenario bound without a receipt; after the
generated target was removed to recover disk space, the exact clean-source
merged-SHA run above passed. This is a device-sensitive resource lane, not a
reason to weaken the bound or hide a failure.

## Complete local verification

These lanes were executed locally on the named device; hosted CI is
corroboration only. The deterministic pipe intentionally skips only the named
10-million-event resource test, which is run separately above.

| Lane | Result | Log SHA-256 |
|---|---|---|
| `scripts/reproducibility-test.sh` on merged `main` | exit 0; pinned inputs, schema, deterministic CLI/export/retention/integrity/authenticated state, recovery MVP, 46 Python tests, Clippy, and all deterministic Rust targets | `c2dff623e985e7441885ee8b0acd8fc78e77b1346442779ceb7d4f348a6576a0` |
| `scripts/offline-network-test.sh` on merged `main` | exit 0; sandbox network canary, privacy regression, full Rust suite, and native resource lane under network denial | `1d83cc7225c65d38fc5d536cc2c4f9499a8efe111e5e7a9c0dc930e6f1e8ad8b` |
| `cargo +1.88.0 fmt --all -- --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `93e71cd3845dab7b20ea39385e3850f02c3ac1c23828fb11639ce523be04eec7` |
| `cargo +1.88.0 build --locked --release` | passed | `ef64a0a99b525c4fae30aab5346a3274b92d333ddd848d17dfa82dbcb0e38706` |
| `cargo +1.88.0 test --locked --release --test recovery -- --nocapture` | 4/4 passed | `3fd264361435f74e4455effc4cc1bf3449140b0f21ec3cd535548d943d207737` |
| `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` | passed | `6de03fdab5309bb6ad95a8446c6b30258b271e9705c53cd4d1ec309d1dc04072` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 69 done, 488 dependency edges | `da78e36f95f6246161becdbb7cbc2e7a1da2ce7a42634efbbceb906ed000a955` |
| `git diff --check` | passed before implementation and evidence commits | no whitespace errors |

The implementation PR's hosted checks also passed both duplicate workflow runs:
CI (Linux stable, macOS stable, Linux MSRV, rustfmt, clippy), Cargo policy,
advisories, dependency review, roadmap, and offline fixture lanes. They do not
replace the local device receipts above.

## Privacy, failure, recovery, and resource boundaries

- Checkpoint signatures are local-key verification, not remote attestation or
  legal chain of custody. Key bytes, payload plaintext, event IDs, and paths
  are not serialized in receipts or manifests.
- Repair never mutates its source. It refuses fixture provenance, empty ranges,
  overlapping ranges, source-mixed ranges, child-orphaning ranges, cursor-tail
  ranges, and intervals over the bounded event limit.
- The repaired copy is independently opened, integrity-checked, authenticated,
  repaired in one transaction, checkpointed again, and shut down with a
  bounded WAL checkpoint. The source is verified again before return.
- The normal writer fail-closed test covers an integrity/foreign-key tamper;
  interval rejection and overlap tests cover negative repair paths.
- FSEvents is a change-notification source and does not prove process causality.
  The native event-storm cursor regression is retained as a gap, not converted
  into a success claim.
- Energy is an explicit no-go when the power counter has no measurable delta.
  Results are comparable only after repeating the same workload on the same
  named device context.
