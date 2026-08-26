# Task 0018 evidence: time-window queries and stable ordering

Status: implementation, review, merge, and protected-main device reproduction
complete.

Implementation PR [#296](https://github.com/AlisinaDevelo/GHOSTRACE/pull/296)
was merged to protected `main` at
`403e1acd00948a8a9619e4ee671fdf2a19d23914` on 2026-08-26. The implementation
commit before squash was `e708487a3659d124d2b744282eb97648977443b6`. This
document is the retained acceptance record for issue #22; the issue is closed
only after this evidence change is merged and linked.

## Contract and acceptance mapping

`QueryRequest` now binds an optional opaque `RootId` alongside the policy
profile, source, kind, observed-time bounds, and page size. Query tokens bind
the complete serialized request; the contract version is `3`, so tokens from
the former request shape fail closed as stale. SQL narrows metadata candidates
inside the authenticated ingest snapshot, then encrypted payloads are decoded
for semantic root matching. Pagination continues from the final
`(observed_at, ingest_seq, event_id)` key, so unmatched roots cannot create
duplicates or skips. `policy_blocked_summary` is a coverage marker, never a
query event.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Queries filter by time, source, root, and kind. | `QueryRequest::matches_event` applies all four filters; the journal preserves SQL source/kind/time narrowing and performs root matching only after payload decryption. `tests/query_pagination.rs::source_kind_root_and_time_filters_are_exact_before_pagination` checks an exact filesystem/root/time result, while `root_filter_paginates_only_matching_roots_and_never_returns_policy_blocked_data` paginates two matching roots past a non-matching root. | PASS |
| Results order by observed time and then ingest sequence. | The query cursor retains the existing total key `(observed_at, ingest_seq, event_id)` and scans ordered rows until `page_size + 1` semantic matches. `tests/clock_order.rs::database_and_export_use_the_same_stable_order`, `ordering_contract_is_versioned_and_total_with_equal_timestamps`, and the explanation property matrix cover equal timestamps, permutations, and page boundaries. | PASS |
| Clock skew is handled explicitly. | The ordering contract never treats display order as causality; known source time is ordered first, durable ingest sequence is the deterministic fallback, and explanation coverage labels missing/ambiguous temporal facts. The merged all-target matrix includes `temporal_fixture_covers_clock_adjustments_and_missing_source_time` and `explanation_labels_ingest_fallback_as_temporal_ambiguity`. | PASS |
| Policy-blocked data is never returned. | Both SQL and semantic matching exclude `policy_blocked_summary`, including an explicit request for that kind. The root-filter test proves the event page is empty while coverage retains `CoverageStatusKind::PolicyDenied`; the determinism property baseline was updated to model the public event-page contract. | PASS |

The root filter is an opaque identifier comparison. No path, payload content,
token plaintext, or policy-blocked observation is returned. Source-level
coverage markers remain visible under a root filter because hiding a denial or
source boundary would make the result look more complete than it is.

## Protected-main device reproduction

Every post-merge lane below ran from protected `main`
`403e1acd00948a8a9619e4ee671fdf2a19d23914` on the named device. Combined
stdout and stderr are retained in `/tmp` and hashed here.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Fixture | `fixtures/causal-chain.jsonl`, 4740 bytes, SHA-256 `22c4e9df8520cbd5b14b343ea2f6c5cca3bcdf0de34433384f6e069b7984bdc8` |
| Source | protected `main` `403e1acd00948a8a9619e4ee671fdf2a19d23914` |
| Date | 2026-08-26 |

Commands and retained receipts:

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| `cargo +1.88.0 test --locked --all-targets --all-features -j1` | exit 0; all targets passed | `6070485479c173f2bd3fb6ee3e503031b6a0563f8c628713c34141f83e952252` |
| `cargo +1.88.0 test --locked --release --all-targets --all-features -j1` | exit 0; all targets passed | `b99437adf5adba5182062058a17e9af10cbd07e38fb0ff2dfe2270028a93c3fb` |
| `scripts/reproducibility-test.sh` | exit 0; all checks passed | `f7a9a1cf2c1f12f156a46ca42a4ea03efe40e67478a0eb19b61dfbe738c52cc0` |
| `scripts/offline-network-test.sh` | exit 0 under macOS `sandbox-exec` | `739fb48984c49543dfce05b76686c9a9788d02e1ab87cdeb469adb84e7aa0656` |
| `scripts/fsevents-sanitizer.sh` | exit 0; AddressSanitizer integration test passed | `ef02364aeb5ad020ba94f87733a48802eda49f8ea97f3bdf589b8344f89391f0` |

The reproducibility lane includes rustfmt, schema/fixture checks, deterministic
demo/explanation/export comparisons, capture refusal, Python tests, Clippy,
and the Rust suite. The offline lane includes the network canary and privacy
regression suite. The sanitizer lane exercised the native lifecycle integration
test with the nightly ASan runtime.

### MVP query receipt

The post-merge MVP path runs the root-filter/policy-blocked negative case as one
focused test and records resource observations from `/usr/bin/time -l`:

```text
cargo +1.88.0 test --locked --test query_pagination \
  root_filter_paginates_only_matching_roots_and_never_returns_policy_blocked_data \
  -- --exact --nocapture
```

It exited 0 with 1 passed and 4 filtered tests. The combined receipt is
`/tmp/ghostrace-0018-merged-mvp.log`, SHA-256
`b2d898b57a655ea5abb01134866567ba1269e0c991a1f9997423ddc67818dd22`; elapsed
time was 0.61 s and maximum resident set size was 59,932,672 bytes. The
receipt contains only synthetic fixture test output and resource counters.

## Local implementation pipe and hosted review

Before merge, commit `e708487a3659d124d2b744282eb97648977443b6` passed the
same device checks. The implementation log digests were:

| Lane | Log SHA-256 |
| --- | --- |
| all-target/all-feature debug tests | `9ae8e55947f45eed23005fe61ef2a2aa520d9dcb6a3a9e9631a39272fdb21fbe` |
| all-target/all-feature release tests | `835e4c527a44819b021a05303b7806dd5e2220fda762f5f0ef43173a0c625622` |
| Clippy with warnings denied | `64a5ed54eee18dc4311a501d7c146eb18086544c155982d9041a9556bb9addf2` |
| reproducibility pipe | `4300380b45ed64f6a8b2c1c8191d72e53bbc39b938842f2071a82572532364cf` |
| offline network lane | `9d4c3a069dc68fe07cc857a01421dac544234d0d8041ecb87850daf2a993f610` |
| FSEvents sanitizer lane | `1ba5af4dbbc55ad6fd2f5bcdb204232ae6bc8e349c58e47fe8c23449cf443d21` |

PR #296 passed the protected repository gates: rustfmt, Clippy, Linux stable,
Linux MSRV, macOS stable, dependency review, deny, audit, roadmap, and the
network-denied fixture job. It was squash-merged by the repository owner; no
hosted result was used as a substitute for the device reproduction above.

The docs-only closure checks also passed: 46 Python tests (log SHA-256
`9ae70987ad9f89a71a70254f5d865f55bbd5e079f14c87e2a137431bc28e2079`), the
roadmap validator with 160 tasks, 12 milestones, 488 dependency edges, 108
parent edges, and zero blocked tasks (receipt SHA-256
`f040706b304a7fe3989af5fcc03895443bc9333ecc9699189c50742a60bf2381`), and the
release-evidence validator (36 measures, receipt SHA-256
`82ff0862f41c422cc4ad89be1d9fb40f64e2fda7179a690ffadff32994edc3f7`).

## Boundaries and limitations

- Root matching requires decrypting candidate payloads because payloads are
  encrypted at rest. A root filter can therefore scan more rows than an
  equivalent metadata-only query; the snapshot boundary remains finite and
  page output is bounded, but this task does not claim a benchmarked scan-cost
  ceiling.
- The query uses source observation timestamps exactly as recorded. It orders
  equal or skewed timestamps deterministically and labels missing/ambiguous
  temporal facts; it does not repair clocks or infer causality from display
  order.
- Policy-blocked summaries are aggregate source-level coverage markers without
  root identity. They are never exposed as events; coverage keeps the denial
  status visible even for root-scoped requests.
- The device and sanitizer lanes exercise synthetic fixtures and the native
  lifecycle adapter. They do not claim live ambient capture, user data, sleep,
  logout, volume detach, or production Keychain behavior.
- This task does not close the M3 aggregate gate or claim retention residue,
  evidence-backed explanations, or export compatibility beyond the already
  merged predecessor contracts.
