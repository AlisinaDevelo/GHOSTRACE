# Task 0078 evidence: clock skew and deterministic total ordering

Status: implementation, review, merge, and protected-main device reproduction
complete.

Implementation PR [#276](https://github.com/AlisinaDevelo/GHOSTRACE/pull/276)
was merged to protected `main` at
`418e37bd471755d9f40bd1f812dc20f627da7b58` on 2026-08-25. The implementation
commit before squash was `c3f7fd637a2770ee1cd9c8d375b6d03d4b13c2cf`. This
document is the retained acceptance record for issue #82; the issue is closed
only after this evidence change is merged and linked.

## Contract and acceptance mapping

Ordering contract version `1` is defined in `src/ordering.rs`. A known source
observation is ordered by `(source_observed_at, ingest_seq, event_id)`;
`ingest_seq` is the durable local sequence and `event_id` is the final canonical
tie-breaker. An absent source observation time is explicit adapter input and
falls back to ingest sequence after known source times. The same comparator is
used by `Journal::ordered_events`, query SQL and encrypted page-token metadata,
and JSONL export. Local `ingested_at` and optional process-local monotonic
sequence are retained as separate timing evidence and never imply causality.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Ordering keys and tie-breakers are versioned and deterministic across database and export implementations. | `ORDERING_CONTRACT_VERSION` is carried by `StableOrderKey`, the query page token, and `ExportManifest`. `tests/clock_order.rs::database_and_export_use_the_same_stable_order` ingests out-of-order and equal-time events and asserts identical `Journal::ordered_events`, query-page, and export IDs. | PASS |
| Fixtures cover clock rollback, leap adjustments, sleep, equal timestamps, delayed batches, and missing source time. | `fixtures/temporal-ordering-v1.json` is manifest-bound and contains all six named scenarios. `tests/clock_order.rs::temporal_fixture_covers_clock_adjustments_and_missing_source_time` checks the scenario set, rollback, leap-boundary progression, delayed/sleep lag, equal-time fallback, and missing-time fallback. | PASS |
| Explanations label temporal ambiguity whenever order depends on ingest sequence rather than source evidence. | `analyze_temporal_observations` emits bounded ambiguity warnings for missing source time, equal timestamps, rollback, delayed delivery, and monotonic regression. `tests/clock_order.rs::explanation_labels_ingest_fallback_as_temporal_ambiguity` verifies the explanation includes both `temporal ambiguity` and `ingest sequence`. | PASS |

The fixture is synthetic and offline. It does not claim that this task triggered
an interactive clock change, sleep, wake, logout, or any other disruptive
device transition; the named cases are deterministic timing-contract inputs.

## Protected-main device reproduction

The focused matrix was rerun from a fresh detached checkout of protected `main`
`418e37bd471755d9f40bd1f812dc20f627da7b58` on the named device. Both commands
ran all four clock-order integration tests and exited 0.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Source | protected `main` `418e37bd471755d9f40bd1f812dc20f627da7b58` |
| Date | 2026-08-25 |

Commands:

```text
cargo +1.88.0 test --locked --test clock_order -- --nocapture
cargo +1.88.0 test --locked --test clock_order --release -- --nocapture
```

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| protected-main debug clock-order matrix | 4 passed, exit 0 | `88f6b6f74bac6442db5fbd5b88461c6e26a9d3615264c1d6248970910f97d755` |
| protected-main release clock-order matrix | 4 passed, exit 0 | `a2ce8d4b666f6685e04389632842b35edb20a9c789a5d46405bc109fcab5b46c` |

The matrix uses only checked-in synthetic timing data and private temporary
journals/exports. It retains no paths, file contents, account names, token
plaintext, network payloads, or user timing data.

## Local implementation pipe

Before merge, the implementation branch ran the full local pipe on the same
device. Every lane exited 0.

| Lane | Log SHA-256 |
| --- | --- |
| red focused compile before implementation (expected failure) | `a5147ee3b183c65b30b59d3a3b4f3a1345024aed682b7525ab76d54680dcdf5a` |
| focused clock-order tests after implementation (4 passed) | `f429f98b1a63448bbbde0530fe299061fae0886312af630174ae53e134352ea0` |
| `cargo +1.88.0 fmt --all -- --check` | `abe44989a66e824392be0e6a5d093408a39f774d2c9089af5980e6d45aa62930` |
| all-target/all-feature Clippy with warnings denied | `d5faa6d1a7ae26011fb0ca2c9c14b116013ff16930d9445073d9b79031e5e338` |
| all-target/all-feature debug Rust tests | `21fa91491df5941ededb51268f3f5d00a2077753b58563c667029e8ba3529c16` |
| all-target/all-feature release Rust tests | `70b6247f94805281ebf541d004b7b5803dcfcda5fd3af03c21e8bdd0b293b3ae` |
| Rustdoc with warnings denied | `7f79ff05c41a9291a0b3201fc605c412522702229d906a8e92f873eec616a59b` |
| Python unit suite | 46 passed; `c937ce7e98e0c8135de2dd498161a972b8a19b602ca0274d4a5c4db69737f78b` |
| fixture manifest (6 fixtures) | `4f286ac8a328a1a49e143a3dc1633bc9e8213075cd046b572958204a3889c095` |
| roadmap validator | `fcbe61ed17d4a85e0d3cb1c6088cc9ec2cf74063f0958d42e54f767305a94867` |
| release-evidence validator | `82ff0862f41c422cc4ad89be1d9fb40f64e2fda7179a690ffadff32994edc3f7` |
| reproducibility contract check | `b9f8618250ede0298c53b787ccec872aa74836471030c7b0c177c3f14422b148` |
| full reproducibility pipe | `f752f0fad9e3191f2f771fd46e53bbed7f6d7e6427de90f3da598916ba7f8ccf` |
| offline network-denied product pipe | `b65ef6712fcfeefe600874c5fd4ceab9f55ebf48cfc8675a319e8096ef205ea2` |
| FSEvents sanitizer lane | `b039742bf2f3e9f4594f3eb9bfe0951fcc153be979d83cca295dd0ace0f8665c` |

The reproducibility pipe also reran deterministic demo, durable reopen/export,
schema, capture-refusal, roadmap, privacy, and all Rust target lanes. The
offline lane enforced network denial before running its privacy and product
suites. The sanitizer lane completed with its documented six suppressions and
no sanitizer finding.

## Hosted review and merge

PR #276 passed both duplicate CI runs for rustfmt, Clippy, Linux stable, Linux
MSRV, macOS stable, offline fixture, Cargo policy, advisories, dependency
review, and roadmap before the protected-main merge. Hosted checks were used as
review gates only; the device pipe and post-merge device matrix above are the
acceptance evidence.

## Boundaries and limitations

- The ordering contract is a display/query order, not a causal relation or actor
  attribution. A source clock rollback remains a source limitation.
- A missing source timestamp is represented only at the adapter timing boundary;
  the current persisted event envelope still requires its validated
  `observed_at` field. A future adapter must preserve the missing-time marker
  rather than inventing a timestamp.
- Monotonic sequence is optional process-local evidence and is not compared
  across sources or machines.
- Sleep and delayed-batch rows are deterministic fixture cases. No interactive
  device sleep/wake or clock manipulation was performed, so those rows do not
  close any native lifecycle task.
- This task does not close the M3 aggregate gate or claim gap-aware windows,
  retention, deletion audit, correlation rules, or live collectors.
