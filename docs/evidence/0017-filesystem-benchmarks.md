# Task 0017 evidence: filesystem correctness and latency benchmarks

Task 0017 is complete as the parent benchmark publication. The native-safe
storm/lifecycle corpus is implemented and evidenced by PR [#268](https://github.com/AlisinaDevelo/GHOSTRACE/pull/268)
and the reproducible eight-workload benchmark corpus by PR
[#270](https://github.com/AlisinaDevelo/GHOSTRACE/pull/270). This consolidated
receipt was generated from protected `main`
`0d4eab9a3f5d0c7139507805117680dcb332a0df` on 2026-08-26. Issue #21 is closed
only after this evidence change is merged and linked.

## Contract and acceptance mapping

The benchmark contract is the pinned synthetic corpus
[`fixtures/filesystem-benchmark-corpus-v1.json`](../../fixtures/filesystem-benchmark-corpus-v1.json),
validated by `python3 scripts/filesystem-benchmark.py check`. It has eight
device-safe workloads, three repetitions, no network, no file-content reads,
and no retained paths. The native release run exercised 24 scenario runs and
the native lifecycle run exercised three rounds of each safe lifecycle row.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Repeated runs report detection latency and duplicate events. | `python3 scripts/filesystem-benchmark.py run --profile release` emitted 24 latency samples: p50 `290.57820899999996 ms`, p95 `402.713875 ms`, p99 `419.507541 ms`, duplicate rate `0.0`. The report is retained below. | PASS |
| Repeated runs report missing events and ordering behavior. | The benchmark report records a `gap_rate` of `0.7404803749267721` and one typed `cursor_regression` failure rather than hiding a missing interval. The merged-main native lifecycle receipt records 0 source-ID duplicates and 0 ordering inversions across three rounds; the 32-run fixture replay separately reports omission, duplicate, and ordering distributions. | PASS |
| Cursor recovery and explicit gap behavior are measured. | The native lifecycle release receipt records `recovery_successes=3`, `collector_dropped_events=0`, and `collector_transport_duplicates=0`. The benchmark's single cursor regression is surfaced as a bounded gap; no live/continuous claim is made after that failure. | PASS |
| Results are evaluated against documented thresholds. | The release gate below compares the measured report with the corpus/resource limits and parent thresholds, including latency, duplicate/order behavior, recovery, RSS, disk growth, and bounded run time. | PASS |

## Release thresholds and measured result

Thresholds are part of this parent publication and are intentionally scoped to
the named device context. A gap is not silently converted to a pass: its gate
is that every loss is typed and surfaced, while recovery is measured separately.

| Measure | Threshold | Merged-main result | Gate |
| --- | --- | --- | --- |
| Detection latency p95 | `<= 500 ms` | `402.713875 ms` | PASS |
| Duplicate rate in native lifecycle rows | `<= 1%` | `0 / 172 = 0%` | PASS |
| Ordering inversions in native lifecycle rows | `0` | `0` | PASS |
| Classified gap failures | every gap has a typed reason; no unclassified loss | `1 cursor_regression`, gap rate `0.7404803749267721` | PASS |
| Restart recovery | `3/3` safe-row rounds | `3/3` | PASS |
| Peak RSS | `<= 256 MiB` | `15,876,096 bytes` | PASS |
| Journal/directory growth | `<= 8,388,608 bytes` (corpus limit) | `6,261,512 bytes` | PASS |
| Per-scenario run bound | `<= 30,000 ms` (corpus limit) | native test assertion passed for all 24 runs | PASS |

The benchmark report also records CPU user/system time (`1,401.693/750.873
ms`) and `energy_nj=0`; the latter is a telemetry no-go/observation and never
substitutes privileged `powermetrics` access. Results are comparable only after
repeating the same corpus on the same named device context.

## Protected-main device receipts

| Fact | Recorded value |
| --- | --- |
| Source | protected `main` `0d4eab9a3f5d0c7139507805117680dcb332a0df` |
| Hardware | MacBookPro17,1, Apple M1 arm64 (`aarch64-apple-darwin`), 8 GB |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Toolchain | rustc/cargo 1.88.0 (LLVM 20.1.5) |
| Corpus fixture SHA-256 | `8f749a756eaf4ff36e78ddf9d5d8822a13d981f138115463b76e02741923b3cf` |
| Corpus check exit | `0`; log SHA-256 `595d08e6fa07b666881639549d11d57cffed59805bdc6dbc31f315ba1613b821` |
| Lifecycle contract check exit | `0`; log SHA-256 `d8754ba160e3d9c4fff96e4b47f64ee0ea919948890b5df5cd5f69a1b872eb70` |

| Native lane | Result | Retained log SHA-256 |
| --- | --- | --- |
| `python3 scripts/filesystem-benchmark.py run --profile release` | exit `0`; 24 synthetic scenario runs; path/content-free report | `3e7bb27c15f82febb7ff973b4581056c12b71376d312d5258d8d5b25bf925b58` (report), `79bcd0f8b4b9ec2a118d6ba04ff8b6328fe94463e1e224730dddc47c9e5b207a` (stderr) |
| `cargo +1.88.0 test --locked --test fsevents_lifecycle_corpus --release macos::native_safe_storm_lifecycle_runs_publish_loss_order_recovery_and_resource_receipt -- --exact --nocapture` | exit `0`; 3 rounds, 172 observations, 0 duplicates, 0 ordering inversions, 3/3 recoveries, 0 drops | `d2515d80f5fd7a72c4968bd7e7dbf1574f89453e314e9e8899c93efaa1a69084` |

The native lifecycle receipt explicitly leaves sleep/wake, logout, and volume
detach as guarded no-go scenarios. It never substitutes fixture replay or
hosted CI for an interactive device transition.

## Local and hosted verification

The merged SHA was already reproduced by the full local device lanes retained
in the task 0085 evidence receipt (reproducibility, network denial/privacy, and
AddressSanitizer). The repository's roadmap check and Python contract tests also
pass on this exact tree. Hosted CI is corroboration, not a substitute for the
device receipts above.

## Boundaries and limitations

- FSEvents is a change-notification source and does not prove process causality;
  coalescing, delay, and omission are reported rather than hidden.
- The p95/duplicate/order thresholds are a named-device release gate, not a
  cross-machine performance claim or confidence interval.
- Energy is an explicit no-go when the power counter is unavailable or has no
  measurable delta; privileged telemetry is not substituted.
- No path, filename, account name, credential, display title, file content, or
  network payload is retained by the corpus or receipts.
