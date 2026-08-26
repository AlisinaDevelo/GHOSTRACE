# Task 0090 evidence: strict Parquet archive profile

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#314](https://github.com/AlisinaDevelo/GHOSTRACE/pull/314)
was squash-merged to protected `main` at
`e2e811c7ca40d2c4b001166659c7f76a321cb5de`. The profile is a contract only;
no Parquet writer or automatic archive path was introduced.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0090-01 | Column types, nullability, schema evolution, ordering, gap, provenance, and policy mappings are versioned. | `schemas/parquet-archive-profile-v1.json` and `fixtures/parquet-archive-profile-v1.golden.json` define strict profile identity/version `ghostrace.parquet-archive-profile` v1. `src/parquet_profile.rs::ParquetArchiveProfile::validate` enforces exactly 23 declared columns, physical types/nullability, `(observed_at, ingest_seq, event_id)` ordering, explicit gap columns, exact provenance/policy identities, and additive-nullable evolution with new-profile gates for removals/type changes. `tests/parquet_profile.rs::profile_is_strict_versioned_and_matches_schema` validates the JSON Schema, golden, and unknown-field rejection. |
| E-0090-02 | Streaming export and validation remain bounded and reject undeclared or lossy conversions. | `validate_row`, `validate_row_count`, and `validate_rows` validate one row at a time, reject unknown/missing columns, wrong physical types, non-canonical JSON, lossy mappings, and gap-field loss, and enforce 1 MiB rows, 10 million rows, and 64 KiB profile metadata. `tests/parquet_profile.rs` covers valid non-gap/gap rows, missing/unknown/type/non-canonical/oversized failures, row-count bounds, and lossy profile mutations. The future writer must call this contract; no writer is silently implied. |
| E-0090-03 | Threat documentation covers column statistics, metadata leakage, temporary files, compression, and downstream deletion limits. | `README.md`, `docs/ARCHITECTURE.md`, and `docs/PRIVACY.md` state the explicit derived plaintext boundary, disabled dictionary/statistics/page indexes, Zstandard's non-confidentiality, `0600` temporary files, atomic publication, cleanup on failure, untouched source journal, forbidden automatic creation, and external-copy deletion limits. |

## Delivery

- Issue: [#94](https://github.com/AlisinaDevelo/GHOSTRACE/issues/94)
- Implementation PR: [#314](https://github.com/AlisinaDevelo/GHOSTRACE/pull/314)
- Implementation commit before squash: `53ce3ac`
- Protected-main merge: `e2e811c7ca40d2c4b001166659c7f76a321cb5de`
- Verification date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 (25G83)
MacBookPro17,1 / Apple M1 / arm64 / 8 GB
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
merged source revision: e2e811c7ca40d2c4b001166659c7f76a321cb5de
```

## Merged-main device verification

Every command in this section ran from the exact protected-main SHA above.
Hosted checks are corroboration; the retained device logs are the acceptance
evidence.

### Deterministic, privacy, failure, and recovery lanes

- `scripts/reproducibility-test.sh` exited `0`: 16 pinned fixture records, the
  profile CLI/golden comparison, 46 Python checks, rustfmt, clippy with
  `-D warnings`, the network-denial fixture lane, deterministic CLI/export/
  retention/integrity/authenticated-state/recovery flows, and all Rust targets
  except the separately invoked native resource test.
- `cargo +1.88.0 test --locked --release --test parquet_profile -- --nocapture`
  exited `0`; 4/4 profile tests passed in optimized mode.
- `cargo +1.88.0 build --locked --release` exited `0`.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- The profile tests exercise happy paths and negative paths: non-gap and gap
  rows, unknown/missing columns, wrong physical types, non-canonical JSON,
  oversized rows, row-count overflow, lossy conversion, leaky statistics, and
  column-count drift. The native-safe FSEvents lifecycle/resource test also
  passed in the deterministic pipe.

### Native device resource lane

The direct merged-main command was run on the named Mac:

```text
cargo +1.88.0 test --locked --test filesystem_benchmark -- --nocapture
```

It exited `0` after `111.77s`; both tests passed, including all 24 synthetic
scenario runs. The emitted receipt recorded arm64/MacBookPro17,1/macOS 26.6.2,
24 latency samples with a maximum of `29558.343459ms` (under the 30-second
scenario bound), CPU user/system `105054.462/880.0ms`, disk growth `5819072`
bytes, and RSS peak `17121280` bytes. The receipt also surfaced the existing
source-boundary behavior honestly: one `cursor_regression` error in an
event-storm run and one in a git-tree run were represented as gaps; the test did
not turn them into positive evidence or suppress them.

## Hosted review and protected merge

PR #314 was pushed from `feature/parquet-archive-profile` and squash-merged
only after both duplicate workflow runs were green. The 20 live checks covered
Linux stable, Linux MSRV, macOS stable, rustfmt, Clippy, roadmap, Cargo policy/
deny, advisories/audit, dependency review, and both network-denial fixture
lanes. Required status checks were not bypassed.

## Retained artifacts

| Artifact | Result | SHA-256 |
|---|---|---|
| `/tmp/ghostrace-0090-merged-repro.log` | merged-main deterministic pipe, exit 0 | `76de3c55aa68f33208f4b843732906288245da3777639b4ef63203c91ebd01d3` |
| `/tmp/ghostrace-0090-merged-release-build.log` | optimized release build, exit 0 | `a70da5408e7a6da7efe97b23011504833b16a7875bda14543f2504b5b8c7d97a` |
| `/tmp/ghostrace-0090-merged-release-profile.log` | optimized profile tests, 4/4, exit 0 | `c17ddf62e3881059aedc06bca9f419764855de2a4662ea7d0f65e1111b16110a` |
| `/tmp/ghostrace-0090-merged-doc.log` | rustdoc with warnings denied, exit 0 | `fa3bd2beb111331ff4605d2d07472ec3025ca2b039b5407cb57a2c9bdfb46853` |
| `/tmp/ghostrace-0090-merged-native.log` | merged native benchmark, 2/2, exit 0 | `a4f84e8d9c4e17ee83964bf66c0fd81cd01d7390f6430205dcdf4c61b4110865` |
| `/tmp/ghostrace-0090-repro-premerge-final2.log` | premerge clean-machine pipe, exit 0 | `d03dcd77a31c053f1c713346313271efa20d4113170f8b98a2c0b48b007f3cfa` |

## Privacy, failure, and scope boundaries

- The profile is subordinate to the encrypted journal and versioned JSONL
  manifest; it never claims to be canonical storage or a writer.
- Parquet compression reduces size but is not confidentiality. Dictionary
  encoding, column statistics, and page indexes are disabled because metadata
  can leak values or ranges. Parquet encryption is explicitly not assumed.
- Temporary output is required to be mode `0600`, atomically published, and
  cleaned on failure. The source journal is untouched and automatic archive
  creation is forbidden.
- Retention and key destruction do not erase an external archive or downstream
  backups. Future deletion work must identify copy ownership and report what
  remains recoverable.
- Gap, provenance, policy, and conversion failures remain visible and fail
  closed; no lossy coercion or unsupported completeness claim is introduced.
