# Task 0016 evidence: event-storm backpressure and loss accounting

Status: implementation, review, merge, and protected-main reproduction complete.

Implementation PR [#272](https://github.com/AlisinaDevelo/GHOSTRACE/pull/272) was
merged to protected `main` at
`954ec4fb9966f36906db65b141f90dbe0f6d4790` on 2026-08-25. The implementation
commit before the merge was
`cdbf5979110113bd9b3257cbb9d1580f45f271d2`. This artifact is the retained
evidence for issue #20; the public issue is closed only after this document is
merged and the issue body links both records.

## Contract and acceptance mapping

The collector keeps its existing `MAX_PENDING_EVENTS = 4096` cap and now
records cumulative callback overflow, writer-admission state, and durable loss
gaps. The writer has one bounded status-reservation slot and a fixed 16 KiB
status-memory reserve. Normal submissions retain their configured queue and
memory limits; a status gap cannot consume an unbounded emergency queue.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Memory remains bounded under an event storm. | The macOS test injects 4,288 synthetic callback events into a private collector. It records `max_pending_events=4096`, `pending_limit=4096`, and `overflow_events=128`; the pending queue drains to zero. The writer's one reserved status slot is covered by `writer::tests::status_submission_has_one_reserved_slot_when_normal_queue_is_full`. | PASS |
| Sustained synthetic load is measured. | The same test records admitted event count, load duration, loss-record duration, cumulative drops, queue cap, overflow count, and recovery status. Debug and release receipts are retained below on the named device. | PASS |
| Induced drops create an auditable gap and collector status. | The induced overflow emits one durable `callback_queue_overflow` gap, reports `dropped_events=4224`, `overflow_events=128`, and sets `recovery_required=true`. The gap is journaled without a path or content field. | PASS |

## Protected-main device reproduction

The reproduction ran from a fresh detached worktree at the exact protected-main
SHA above, on the same device used for implementation testing. Each command's
combined stdout and stderr is retained in `/tmp` and hashed here.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1) |
| Rust/Cargo | rustc/cargo 1.88.0, host `aarch64-apple-darwin` |
| Python | 3.9.6 |
| Source | protected `main` `954ec4fb9966f36906db65b141f90dbe0f6d4790` |
| Date | 2026-08-25 |

Command (debug):

```text
cargo +1.88.0 test --locked --lib fsevents_collector::tests::synthetic_event_storm_backpressure_stays_bounded_and_emits_durable_gap -- --exact --nocapture
```

Command (release):

```text
cargo +1.88.0 test --locked --lib --release fsevents_collector::tests::synthetic_event_storm_backpressure_stays_bounded_and_emits_durable_gap -- --exact --nocapture
```

Both commands exited 0 and ran one test (`39 filtered out`). The post-merge
receipts were:

```json
{"admitted_events":64,"admitted_load_ms":48.529458999999996,"auditable_gap_count":1,"dropped_events":4224,"max_pending_events":4096,"overflow_events":128,"pending_limit":4096,"recovery_required":true,"schema_version":1,"storm_loss_record_ms":0.643792,"synthetic_events":4288}
```

```json
{"admitted_events":64,"admitted_load_ms":25.555917,"auditable_gap_count":1,"dropped_events":4224,"max_pending_events":4096,"overflow_events":128,"pending_limit":4096,"recovery_required":true,"schema_version":1,"storm_loss_record_ms":0.25175000000000003,"synthetic_events":4288}
```

| Lane | Exit | Log SHA-256 |
| --- | ---: | --- |
| post-merge debug stress | 0 | `43516a273a566a62e2c793fd09fbddf77ea000d636c0f20ac8ec078f6a9dc0ed` |
| post-merge release stress | 0 | `ecde1395330af3353e402df9f9e7008cfab78bf49bde42c3f1c2a37970c79422` |

The post-merge run was performed with the test's private mode-0700 temporary
root. It does not retain paths, filenames, account names, file contents, or
network payloads.

## Local implementation pipe

Before the merge, the implementation branch ran the complete local pipe on the
same device. Every lane exited 0:

```text
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.88.0 test --locked --all-targets --all-features
cargo +1.88.0 test --locked --all-targets --all-features --release
cargo +1.88.0 build --locked --all-features --release
cargo +1.88.0 doc --locked --all-features --no-deps
python3 -m unittest discover -s tests -p 'test_*.py'
python3 scripts/fixture-manifest.py check
python3 scripts/filesystem-benchmark.py check
scripts/reproducibility-test.sh
scripts/offline-network-test.sh
scripts/fsevents-sanitizer.sh
```

The combined lane log hashes are:

| Lane | Log SHA-256 |
| --- | --- |
| release build | `11f6631ce1aa3d114d27ec7041563696b5bf52ad6c6c73b98ca470fb8b002dcc` |
| clippy | `52135c797f12bfbe42d643d3d75d0b10c27de9bbbcf3ab118deb4519115f461c` |
| corpus check | `595d08e6fa07b666881639549d11d57cffed59805bdc6dbc31f315ba1613b821` |
| fixture check | `bff8fb4531b2932ae572808ab57b81d18200bbb3fa9b5d09a53af1706bb7e73f` |
| format | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| offline network | `e303e1900f04b2cab4068283bea604fa9bccd580bf4e243bc7843e986457828d` |
| Python tests | `d54159bfbbac938aa2d96ff8aee10ee7252901fbed2ab86de8e1a6a021761e00` |
| reproducibility | `36f6d5b42b6fbd5abc26366edb5cd3898668805c08c2b0fb835b4337920cfe2c` |
| rustdoc | `78dc75e4681edd1b64543461c98fecc9841b6b1182a609f7aace48458e908bff` |
| FSEvents sanitizer | `93fa1651cf3bc4ac3ea1e71ae58b4b4c795ceb41c0ccd8f7d37954049d576d28` |
| debug all-target tests | `1416ccc39fc184f7f5d194980c1d2016b07284ab9f8f52210ee494dcec1cb86a` |
| release all-target tests | `5d8149cddb3727cc2ee0aba631ae4b182a878b8a0186acab3e0653f153013a5d` |

The red test-log SHA-256 is
`241f420951c9ac89f05a1aa647969a6fe5ac2ac8d6f51dd72f39723862407d59`.
The pre-implementation run failed to compile because the status counters and
reserved status-admission API did not yet exist; implementation followed that
red result.

Hosted pull-request checks corroborated the merge, but they are not used as a
substitute for the local device pipe or the protected-main reproduction.

## Boundaries and limitations

- The stress workload is synthetic and injects callback events directly into a
  private collector queue. It proves the bounded admission/loss contract, not
  ambient filesystem throughput, energy, or cross-machine performance.
- The induced loss is intentionally represented as a durable gap with
  `recovery_required`; it is never converted into a false continuous history.
- One emergency status slot and a fixed memory reserve are bounded by design.
  Exhaustion of that reservation is fail-closed and remains visible as a gap
  admission error.
- Sleep/wake, logout, and volume-detach behavior remain separate guarded
  no-go cases in the M2 aggregate gate. This task does not mark M2-001 or
  M2-002 observed; M2-003 is the only aggregate measure currently observed.
