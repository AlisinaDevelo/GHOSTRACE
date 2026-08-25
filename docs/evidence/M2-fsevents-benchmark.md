# Task 0076 evidence: reproducible filesystem benchmark corpus

Status: implementation, review, merge, and protected-main device verification
complete. The implementation PR [#270](https://github.com/AlisinaDevelo/GHOSTRACE/pull/270)
merged to protected `main` at
`e1f6a14ef445f5068dd8c52c9188c3a0e4ad41a0`. This evidence is being published in
a separate docs/ledger PR; the public issue is closed only after this artifact
is merged and its final reproduction is verified.

## Contract and acceptance mapping

The locked corpus is
[`fixtures/filesystem-benchmark-corpus-v1.json`](../../fixtures/filesystem-benchmark-corpus-v1.json)
and its manifest entry. `python3 scripts/filesystem-benchmark.py check` enforces
the exact schema, deterministic generator seed, three repetitions, eight
scenario IDs, synthetic-only/no-network/no-content-retention privacy contract,
and bounded entries, file bytes, run time, and journal growth.

| Acceptance criterion | Retained evidence |
| --- | --- |
| Small, deep, wide, Unicode, case-variant, Git, build-output, and event-storm trees without user data | Eight ordered `device_safe` scenarios in the fixture; native Rust generators create a private mode-0700 temporary root, use synthetic bytes only, and assert that receipt output contains no path or content fields. Fixture digest: `8f749a756eaf4ff36e78ddf9d5d8822a13d981f138115463b76e02741923b3cf` (4613 bytes). |
| Latency percentiles, coverage classes, duplicate/gap rates, CPU, memory, energy, and disk growth | The path/content-free report includes p50/p95/p99 over 24 samples (three runs per scenario), direct/contextual/inferred/unknown counts, duplicate and gap rates, CPU user/system time, peak RSS, energy counter delta, and journal/directory growth. |
| Hardware/OS context and no unsupported cross-machine claim | Every receipt names model, OS, architecture, and toolchain. The report limitation permits comparison only after repeating the same workload on the same named device context; no cross-machine normalization claim is made. |

## Protected-main target-device reproduction

The receipt was generated from a fresh detached worktree at the exact merged
SHA, using `python3 scripts/filesystem-benchmark.py run --profile release`.
The command exited `0`; the native Cargo test ran 3 repetitions of all 8
scenarios (24 scenario runs). The command's report and stderr are retained in
the local evidence store:

| Fact | Recorded value |
| --- | --- |
| Source | protected `main` `e1f6a14ef445f5068dd8c52c9188c3a0e4ad41a0` |
| Hardware | MacBookPro17,1, Apple arm64 (M1) |
| OS | macOS 26.6.2 |
| Toolchain | rustc 1.88.0 (6b00bc388 2025-06-23) |
| Report | `/tmp/ghostrace-0076-merged-release-report-verified.json`, SHA-256 `a427ee54233b58869e25dca282a76defd6db290192f142c5d485fe8b56cdd733` |
| Native command stderr | `/tmp/ghostrace-0076-merged-release-verified.stderr`, SHA-256 `296411e7cd6585b2f87228938a4e84f6a825cfac5ec16506f267935485b03750` |
| Native command exit | `0` |

The merged-main report was:

```json
{
  "coverage_classes": {"contextual": 379, "direct": 973, "inferred": 0, "unknown": 0},
  "device": {"arch": "arm64", "model": "MacBookPro17,1", "os": "26.6.2", "toolchain": "rustc 1.88.0 (6b00bc388 2025-06-23)"},
  "duplicate_rate": 0.0,
  "failure_counts": {"cursor_regression": 1},
  "gap_rate": 0.6865846514352666,
  "latency_percentiles_ms": {"p50": 295.375125, "p95": 405.876917, "p99": 407.548291},
  "resource": {"cpu_system_ms": 712.533, "cpu_user_ms": 1090.482, "disk_growth_bytes": 6138656, "energy_nj": 0, "rss_peak_bytes": 14729216},
  "scenario_count": 24,
  "source_revision": "e1f6a14ef445f5068dd8c52c9188c3a0e4ad41a0"
}
```

The event-storm workload retained one observed `cursor_regression` as a bounded
failure and gap instead of aborting, hiding it, or claiming continuity. Energy
is reported as `0` for this run because no measurable accumulated power-counter
delta occurred at the sampling boundaries; the report retains that limitation
and never substitutes privileged `powermetrics` output.

## Complete local verification before implementation merge

The implementation branch passed the full local pipe on the same device before
PR creation. These are exact command lanes, not claims inferred from hosted CI:

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| `cargo +1.88.0 fmt --all -- --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `e1586b2e3f8d9e35e4a65081c67e252e72c3ec2af68881eb6b35ee91a5ebd5b9` |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed | `bff8fb4531b2932ae572808ab57b81d18200bbb3fa9b5d09a53af1706bb7e73f` |
| `cargo +1.88.0 test --locked --all-targets --all-features --release` | passed | `c067e929df22d40dbfd41f62a522ba35f4e266db52535c9c19ea96c4569e5883` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `bf8c8eb5603bb906b695dc9574f87e68c84ae5e93ed0eb4021c21cd80138bf1f` |
| `cargo +1.88.0 doc --locked --all-features --no-deps` | passed | `313ed6f0f81b8b8d28a2580741f4dd44e8dc67a8e0a572aceefc69deb0461e94` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | 46 passed | `f083609ea1faa0c6c72a5c251928780e8a522f9ba11e277e32023eac91a31c72` |
| `python3 scripts/fixture-manifest.py check` | passed; 5 fixtures | `bff40324a8738623f3b897d9c8b7a67ad5cc24efd50850d0705f2357704010de` |
| `python3 scripts/filesystem-benchmark.py check` | passed; 8 scenarios | `595d08e6fa07b666881639549d11d57cffed59805bdc6dbc31f315ba1613b821` |
| `scripts/reproducibility-test.sh` | passed | `931830e5f51f1e3ebf3ab1858e16fa293de5730c168d515a48fbbbadbfa29a56` |
| `scripts/offline-network-test.sh` | passed | `47949dc3f0daa8814bc0687906175eb9e93bc5b37ad2cf0d256105ed7a1eb7d6` |
| `scripts/fsevents-sanitizer.sh` | passed | `bb015f5783c5e1b9d050e80208e81b4c721939d95f66a0e7761117014f35cc5a` |

Hosted PR checks also passed (CI, macOS/Linux/MSRV, rustfmt, clippy, audit,
dependency review, Cargo policy, roadmap, and offline fixture lanes), but they
are corroboration rather than a substitute for the target-device receipt.

## Boundaries and limitations

- FSEvents is a change-notification source and does not prove process causality.
- The event-storm cursor regression is retained as a gap; a non-zero gap rate is
  an observed result, not silently converted to a pass.
- Energy is a no-go when the power counter is unavailable or has no measurable
  delta; this run records `energy_nj: 0` and the limitation above.
- The three-repetition sample distribution (p50/p95/p99) is the benchmark's
  uncertainty report; it is not a confidence interval and must not be compared
  across machines without a separately normalized study.
- No path, filename, account name, credential, display title, file content, or
  network payload is retained by the corpus or receipt.
