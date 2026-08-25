# Task 0079 evidence: gap-aware query windows

Status: implementation, review, merge, and protected-main device reproduction
complete.

Implementation PR [#278](https://github.com/AlisinaDevelo/GHOSTRACE/pull/278)
was merged to protected `main` at
`71b7b441874b606ae5558f58c97aaff4f17b90f4` on 2026-08-25. The reviewed
implementation commit before squash was
`2532d62f968a56d54aff40df872ad6290a25547c`. This document is the retained
acceptance record for issue #83; the issue is closed only after this evidence
change is merged and linked.

## Contract and acceptance mapping

Query coverage contract version `1` is returned with every query page unless a
caller explicitly sets `include_coverage=false`. Coverage discovery is bounded
to `MAX_COVERAGE_MARKERS` and deliberately ignores the event-kind filter so a
sparse query cannot hide a relevant marker. Gap intervals preserve an explicit
open end when the source has no end boundary. Query token contract version `2`
binds the matching-row count used to detect retention deletion between pages.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Coverage reports distinguish no events observed, source disabled, policy denied, source gap, retention deletion, and unknown history. | `CoverageStatusKind` and `QueryCoverage` implement all six states plus observed events. `tests/query_gap_coverage.rs::query_distinguishes_no_events_unknown_history_and_explicit_markers` exercises no-event, unknown-history, source-disabled, policy-denied, and source-gap markers; `continuation_detects_retention_deletion_and_opt_out_is_explicit` deletes a retained row after the first page and verifies the retention status. | PASS |
| Window and source filters cannot hide a relevant gap without an explicit opt-out marker in the response. | Marker scans ignore `kind` and SQL source filters, then apply semantic source/window intersection. `query_reports_markers_even_when_kind_filter_would_hide_them` verifies a filesystem gap survives a filesystem-event kind filter. `include_coverage=false` returns `coverage.opted_out=true`. | PASS |
| Golden tests cover nested, adjacent, open-ended, and cross-source gap intervals. | Manifest-bound `fixtures/query-gap-coverage-v1.json` contains the four named cases. `golden_gap_intervals_are_versioned_and_deterministic` checks the schema and expected cases; interval integration tests cover inclusive adjacency, open ends, and source mismatch. | PASS |

The coverage fixture and all query tests are synthetic, offline, and path-free.
They do not claim that this task exercised an interactive sleep, logout, volume
detach, or native collector interruption; those remain explicit guarded rows in
the support and lifecycle matrices.

## Protected-main device reproduction

The focused matrix and full product suites were rerun from protected `main`
`71b7b441874b606ae5558f58c97aaff4f17b90f4` on the named device. Every command
exited 0.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Source | protected `main` `71b7b441874b606ae5558f58c97aaff4f17b90f4` |
| Date | 2026-08-25 |

Commands:

```text
cargo +1.88.0 test --locked --test query_gap_coverage --test query_pagination -- --nocapture
cargo +1.88.0 test --locked --release --test query_gap_coverage --test query_pagination -- --nocapture
cargo +1.88.0 test --locked --all-targets --all-features
cargo +1.88.0 test --locked --all-targets --all-features --release
```

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| protected-main focused debug coverage + pagination | 8 passed, exit 0 | `8b627941e2e345f964909c503e3caafc0dad397726e3d3e611d0bda0e87215a2` |
| protected-main focused release coverage + pagination | 8 passed, exit 0 | `8f925a2287d54704b880a82535f080fb1d4f812024ab6e9d38712c3c380ed08e` |
| protected-main all-target/all-feature debug | all targets passed, exit 0 | `f054d299c7c4a8a31b720c18270e8deb1c1ec14298fc0120c8355a34eeb85ffb` |
| protected-main all-target/all-feature release | all targets passed, exit 0 | `fe84d7f42386c2d6e8f166d9095772706ab723b8555e6c4bc44eca6b07bcfd93` |

The focused and full suites use private temporary journals and checked-in
fixtures. They retain no paths, file contents, account names, token plaintext,
network payloads, or user timing data.

## Local implementation pipe

Before merge, the implementation branch ran the local pipe on the same device.
Every required lane exited 0 after the expected red test drove the API shape.

| Lane | Log SHA-256 |
| --- | --- |
| red focused compile before implementation (expected failure) | `15b0d8dd9a7bc383f2ffb7ad79735210c80ab02b78883e23ab5f9d33bb4149fa` |
| focused coverage + pagination tests (8 passed) | `83742862c4513d21fbceca94eaf311aefb0ecc1064a87c9bf0689ffd8d870fbf` |
| all-target/all-feature debug Rust tests | `0fa5d95bdfca4274b43055c04b7c54d77adc8575a6c2bb9fc266fbd629a33f6a` |
| all-target/all-feature release Rust tests | `5b9011fe3285541b31d3e31e5c2cea43aab82852d9fe42eb88526ad23772ae9c` |
| all-target/all-feature Clippy with warnings denied | `7eaaf8f3aaaf654a322dfb28040e5bd8a17e0128939dac2f48feee1761f4d517` |
| Rustdoc with warnings denied | `8f587344ef98defbb209c434e8823ba3e46a476963fdc9e9611c03c1ecc59220` |
| Python unit suite (46 passed) | `64ffc1c7ffb234ca2965f108d571911c583710bbc8a18f0fdece1575dee0e4f8` |
| fixture-manifest, roadmap, and release-evidence validators | checked by the focused pipe; all returned `ok` |
| reproducibility contract check | `570d1b5b366745878ba2f4caf37f4897f5344d7bd621b08af25cfd57a48796c4` |
| full reproducibility pipe | `68f9386c1a875ef4940acc45a674a50963492bfe867ebe853246e1c2660ed7e8` |
| offline network-denied product pipe | `902ae08d11f30cd40ee062bf016280f9bcdfc87bfa6be85384d83b6b29ab81f5` |
| FSEvents sanitizer lane | `ae7920703a576eb47047570eaf84db96a932ba02b099dc7719e98a759d2dc286` |

The reproducibility pipe reran deterministic demo, durable reopen/export,
schema, capture-refusal, roadmap, privacy, and all Rust target lanes. The
offline lane enforced network denial before the privacy and product suites. The
sanitizer lane completed with its documented suppressions and no sanitizer
finding. The two Keychain lifecycle tests remain explicitly ignored because
they require separate interactive authorization; that limitation is not used
as evidence for this query contract.

## Final main validators

The merged-main checkout also passed the non-Cargo contracts and documentation
lanes:

| Lane | Log SHA-256 |
| --- | --- |
| `cargo +1.88.0 fmt --all -- --check` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| protected-main Clippy | `9b479d684d379a217435d6aa6e6a08d1662310311712862bfd18caa0b993c57a` |
| protected-main Rustdoc | `f3a2f5ea43129ee0b751c577cc367be8bf9f4ef54b7b351389bc08332c71bc4e` |
| protected-main Python suite (46 passed) | `7b09dd4aef5879cd6e5c4bdb2b12dfd2776c87dae9841cc0618bf5d742bb5c5d` |
| fixture, roadmap, release-evidence, and reproducibility contracts | `169ccd1696d377d19edf3f4226e9bd44b3263efbb6699379dbb3ecb9e60699c2` |
| protected-main offline-network product pipe | `cc1a555f33e3f749c69f527bc8f74b443413ac9a001df4402377edefe7b355cb` |

## Hosted review and merge

PR #278 passed both duplicate hosted runs for rustfmt, Clippy, Linux stable,
Linux MSRV, macOS stable, offline fixture, Cargo policy, advisories,
dependency review, and roadmap. Hosted checks were review gates only; the
device pipe and post-merge device matrix above are the acceptance evidence.

## Boundaries and limitations

- Coverage markers are bounded to prevent an unbounded query response; a
  truncation flag is returned when the bound is exceeded.
- An open-ended gap is conservative: it remains intersecting until a later
  source observation supplies a bounded recovery boundary.
- Retention deletion detection covers rows removed after a continuation token
  was issued; it does not reconstruct deleted payloads.
- Unknown history is intentionally conservative when no recognized marker or
  observed event explains an empty window.
- This task does not close the M3 aggregate gate or claim native collector
  completeness, cross-source correlation, or release readiness.
