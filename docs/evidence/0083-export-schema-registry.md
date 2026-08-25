# Task 0083 evidence: export schema and manifest registry

Task 0083 is implemented, reviewed locally, merged to protected `main`, and
reproduced on the named device. Implementation PR [#289](https://github.com/AlisinaDevelo/GHOSTRACE/pull/289)
merged at `4d6b758c37e86616893297602a666531a8e39eb0` on 2026-08-25. The reviewed
implementation commit before squash was `4d722053d7f7c6386164bc3040f79fd7c98d6e52`.
Issue #87 is closed only after this evidence change is merged and linked.

## Contract and acceptance mapping

[`schemas/export-registry-v1.json`](../../schemas/export-registry-v1.json) is
the authoritative registry for six v1 record contracts:

| Record | Stable schema ID | Schema | Golden |
| --- | --- | --- | --- |
| Manifest | `ghostrace.export-manifest` | `schemas/export-manifest-v1.json` | `fixtures/export-manifest-v1.golden.json` |
| Event | `ghostrace.event-envelope` | `schemas/event-envelope-v1.json` | `fixtures/event-envelope-v1.golden.json` |
| Gap | `ghostrace.gap-record` | `schemas/gap-record-v1.json` | `fixtures/gap-record-v1.golden.json` |
| Claim | `ghostrace.claim-record` | `schemas/claim-record-v1.json` | `fixtures/claim-record-v1.golden.json` |
| Policy | `ghostrace.policy-record` | `schemas/policy-record-v1.json` | `fixtures/policy-record-v1.golden.json` |
| Source coverage | `ghostrace.source-coverage` | `schemas/source-coverage-v1.json` | `fixtures/source-coverage-v1.golden.json` |

All descriptors declare compatibility class `strict` and reject unknown fields.
The registry document schema is separately checked by
`schemas/export-registry-schema-v1.json`. The registry JSON SHA-256 is
`a48749d56a7884822ddecbe74d7094dcb2f1ad3d50b55c4af292d06092dd1284` (1,970
bytes); the fixture manifest records every golden's size and digest.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Every schema has a stable identifier, compatibility class, strict unknown-field behavior, and golden examples. | `SchemaRegistry` validates the six closed descriptors, duplicate IDs/types, v1 versions, strict compatibility, safe repository-relative schema/golden paths, and registry schema shape. `tests/export_schema.rs::registry_is_versioned_strict_and_every_schema_has_a_golden` compiles every JSON Schema, validates every golden, validates the registry document, and injects an `unknown_field` into each contract; all six reject it. | PASS |
| A manifest binds record counts, byte counts, digests, schema versions, query scope, policy profiles, gaps, and tool version. | `ExportManifest` now carries registry/schema/export/tool versions, a schema-version map for all six contracts, deterministic `all_committed` query scope, policy profiles, coverage and gap records, and body maps for event/gap/claim/policy/source-coverage counts, bytes, and SHA-256 digests. Body accounting intentionally excludes the manifest line to avoid a self-referential digest. `tests/export_schema.rs::export_manifest_binds_schema_versions_counts_bytes_and_digest` checks the generated manifest and validated result. | PASS |
| Validators reject mixed or undeclared versions before any consumer treats an export as complete. | `validate_export` parses the manifest first with strict deserialization, checks the checked-in registry and exact versions, then accepts only declared event records in the shared `(observed_at, ingest_seq, event_id)` order. It rejects unknown fields, mixed IDs/versions, duplicate/order regressions, unsupported scope, policy/gap/collector drift, and count/byte/digest drift. The CLI `validate --export` and `export_journal` use this gate. `tests/export_schema.rs::export_validator_rejects_mixed_versions_unknown_fields_and_digest_drift` covers mixed-version, reordered, unknown-field, and digest mutations. | PASS |

The export fixture produced on merged main contained 9 JSONL lines (one
manifest plus eight events), 8 event records, 5,602 event-body bytes, coverage
`event_count=8`, `gap_count=1`, and event body digest
`83a11a7f56b5e3ff3a2495392e4a200c66423c60d2e4e08f203ee88c0a03a995`.
The complete export file digest was
`bbfca31a7425fc12560439c9c8f7662053776eacf44351a0da963ef24335b6ef`; export
and CLI validation both exited 0.

## Protected-main device reproduction

The exact merged SHA was rerun on the named device. Every required lane below
exited 0; logs remain in `/tmp` with digests so later reproduction can detect
drift.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Source | protected `main` `4d6b758c37e86616893297602a666531a8e39eb0` |
| Date | 2026-08-25 |

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| `scripts/reproducibility-test.sh` | all checks passed, exit 0 | `ff8132093fa1eae587432d3c18f4a572b2ff9af0800d5fc900214ef26e00b583` |
| `scripts/offline-network-test.sh` | sandbox network denial, canary/privacy/product suite passed, exit 0 | `177843add1099e2ede763bd66b2688aea6394e3f8164fbf4022e4fd8ea3a8120` |
| `scripts/fsevents-sanitizer.sh` | native lifecycle test passed with no sanitizer finding, exit 0 | `e5636032b94e6fed9d74615f18981ef15162bfb552ebf4b04e720e7f7a2690df` |
| Release focused export/CLI tests, fmt, Clippy, validators, 46 Python tests, diff check | all passed, exit 0 | `ab53642ab62fbec88b4e246b6584c677b3691617d56b763622a4b5c2bfc6f0b5` |
| Manual merged-main export plus `ghostrace validate --export` | 8 exported and 8 validated events, exit 0 | file digest recorded above |

The full reproducibility pipe exercised the fixture demo, durable reopen,
deterministic export, the new validator CLI, capture refusal, native-safe
FSEvents tests, privacy regressions, all Rust targets/features, Clippy, fixture
and roadmap validators, and 46 Python tests. No user paths, account names,
credentials, payload contents, or network data were retained or echoed.

## Hosted review and merge

PR #289 had 19 successful hosted check entries across duplicate push and
pull-request runs: rustfmt, Clippy, Linux stable/MSRV, macOS stable, offline
fixture, Cargo policy, advisories, dependency review, and roadmap. The observed
workflow runs were `32897804658` and `32897827021`; the merge state was `CLEAN`
before squash. Hosted checks were a review gate; the device lanes above are the
acceptance evidence.

## Boundaries and limitations

- The six contracts are v1 and intentionally strict; additive compatibility is
  a future registry decision, not an implicit reader behavior.
- Current JSONL output emits manifest and event records. Gap, claim, policy, and
  source-coverage contracts are published and counted as zero body records until
  their dedicated streaming/export tasks emit them; gap facts remain bound in
  the manifest coverage section.
- Fixtures and goldens are synthetic, offline, and network-free. This task does
  not enable live capture, claim causality, or close the M3 aggregate gate.
- Sleep/wake, logout, volume-detach, and interactive Keychain lifecycle rows
  remain explicit no-go or ignored rows and were not converted into passes.
