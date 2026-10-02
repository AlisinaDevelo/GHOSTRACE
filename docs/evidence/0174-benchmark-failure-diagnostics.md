# Task 0174 evidence: native benchmark failure diagnostics

Diagnostic acceptance is complete using the task's recorded-failure alternative.
This is not a ten-consecutive-pass or performance-readiness claim. Implementation
PR [#410](https://github.com/AlisinaDevelo/GHOSTRACE/pull/410) was reviewed and merged
to protected main at `9aa1cdee25ebbe04bb6628c61bd3ae35597350ef`. The clean candidate
`50082947d9a42d09a9b10e38e620332ecda0dc31` and merged trees were identical.

Issue: [#395](https://github.com/AlisinaDevelo/GHOSTRACE/issues/395).

## Acceptance mapping

| Evidence | Criterion | Retained result |
| --- | --- | --- |
| E-0174-01 | Each native assertion identifies scenario, run, value and bound. | Coverage, operation/entry bounds, elapsed time and receipt-privacy assertions share the diagnostic helper. A cross-platform test catches the actual assertion panic and verifies all four fields. The actual under-load failure below demonstrates the timing diagnostic. Privacy checks print booleans, not selected paths. |
| E-0174-02 | Deadlines derive from the corpus budget. | The global scenario gate reads `resource_limits.max_run_ms`; the drain window is one tenth of that budget. The checked-in 30,000 ms/3,000 ms limits are unchanged. A synthetic corpus-budget mutation must change both derived limits. Smaller per-scenario corpus metadata is not newly enforced by this change. |
| E-0174-03 | Ten under-load passes, or failure retained with its reason. | The clean candidate under concurrent full-suite/compiler load failed at `event_storm_tree`, run 2: `elapsed_ms=50793`, `max_run_ms<=30000`, exit 101. The raw failure is retained. The same reproduction on the identical merged tree passed all 24 scenario repetitions in 98.68 s alongside the full suite. The optimized merged run passed in 18.23 s. These mixed results are retained; no ten-pass matrix or performance fix is claimed. |

## Device reproduction

Verification date: 2026-10-02 UTC. Reference device: MacBookPro17,1, Apple M1,
macOS 26.6.2 (25G83), arm64. Pinned rustc 1.88.0 (`6b00bc388`, host
`aarch64-apple-darwin`), cargo 1.88.0 (`873a06493`).

The debug reproduction ran on both the clean candidate and exact merged SHA:

```sh
GHOSTRACE_BENCHMARK_REVISION="$(git rev-parse HEAD)" \
  cargo +1.88.0 test --locked --test filesystem_benchmark \
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt \
  -- --exact --nocapture --test-threads=1
```

The full all-target/all-feature suite ran concurrently in a separate worktree
and target directory, excluding only the benchmark being measured separately.
That suite passed, with 390 reported passes. The optimized reproduction adds
`--release` to the same benchmark command. Both merged receipts carry source
revision `9aa1cdee25ebbe04bb6628c61bd3ae35597350ef` and 24 scenario records.

Formatting, target Clippy with warnings denied, all three Rust diagnostic/corpus
tests, all three Python benchmark tests, and the eight-scenario corpus check
passed. The full-suite load included the reviewed consent-text correction and its
regression test; production journal/benchmark algorithms were unchanged.

## Retained artifacts

Raw logs remain outside the repository on the verification device, under audit
ID `ghostrace-audit-20261002` in `ghostrace-evidence/audit-20261002.aaOsOD`.

| Artifact | Result | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| `benchmark-candidate-under-load.log` | Actual named 50,793 ms/30,000 ms failure, exit 101 | `a48f2351597c534ea80e1e3b463cbc316dad0b0cf02a7e589747bee70477fafb` | 874 |
| `benchmark-merged-under-load.log` | Exact merged-main debug rerun, 24 repetitions, exit 0 | `4785cc159793a440bb2dea8831cfcff059f6926be38633f3c10aa8c598469d50` | 5759 |
| `benchmark-merged-native-release.log` | Exact merged-main optimized rerun, 24 repetitions, exit 0 | `5dd9a177fb09b0a4599b6b67af6e609c273b5d705f518ca841f1521c27fbe72d` | 5777 |
| `benchmark-merged-machine.txt` | Source, load and swap capture around the merged run | `9375517d0026716d5461c270611e4cd1e3239ba19e62eae33223b323a91eac3e` | 173 |
| `acceptance-full-suite-final.log` | Concurrent non-benchmark suite, exit 0 | `e492d2401b2b48c26f9ccb6c5e9e69b193711ba953df1ccd58d633397b0073de` | 45104 |

## Limits and no-go results

- The failure reason is the breached scenario runtime gate. Load and debug versus
  optimized execution are recorded context, not a causal profiling conclusion.
- Ten consecutive under-load green runs were not completed. The explicit
  recorded-failure alternative is used, and the intermittent performance result
  remains visible.
- No limits were relaxed, no failure was discarded, and no large-journal result
  is inferred from this small synthetic corpus. Incremental authenticated-state
  and 100,000-event lock/performance work remain open in #403/task 0175.
- An earlier dirty-harness failure existed only in a terminal transcript. It is
  not used as acceptance evidence; the independently retained clean-candidate
  failure above replaces it.
