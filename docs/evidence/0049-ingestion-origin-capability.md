# Task 0049 evidence: explicit ingestion origin capability

Status: complete.

The journal ingestion boundary now requires a typed `IngestionOrigin` capability.
Provenance versions and collector-instance namespaces are owned by the adapter
boundary instead of being accepted as caller-supplied strings.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0049-CODE-E1C3F8A` — `e1c3f8a` |
| Implementation pull request | [#183](https://github.com/AlisinaDevelo/GHOSTRACE/pull/183) |
| Protected-main merge | `GHOSTRACE-0049-MERGED-E926547` — `e926547e1790cbf80afb4ac5aaefd71ed98e778e` |
| Post-merge local pipeline log | `GHOSTRACE-0049-POSTMERGE-LOG-20260824` — SHA-256 `ac22cc8b788d9acc1583639bc51d3d1daeb935bc6810c4512fdf1cd51ffb091b` |

The raw log is retained locally only. It contains platform and tool versions,
test output, and no event payload, fixture contents, credentials, account data,
network service data, or private host identifiers.

## Boundary implemented

- `Journal::ingest` and `Journal::ingest_batch` require an `IngestionOrigin`.
- `Fixture`, `Live`, `Import`, and `Repair` are distinct origin kinds.
- Fixture provenance is fixed to `fixture-v1` and the `fixture-` collector
  namespace; fixture ingestion remains compatible with deserialized JSONL.
- Live, import, and repair constructors are crate-owned and use private
  capability tokens that are skipped by serialization.
- Import rejects lifecycle assertions; repair accepts only gap,
  policy-blocked-summary, and source-error classes.
- An event constructed in memory retains its capability binding. A deserialized
  envelope has no live/import/repair binding and must not be promoted by the
  generic journal API.

## Acceptance evidence

The regression `deserialized_fixture_cannot_claim_live_collector_identity`
rewrites a valid fixture envelope to `live-filesystem-1`/`live-v1`, deserializes
it successfully, and proves that the public fixture capability refuses it before
policy or persistence. The test also verifies that the journal remains empty.

The model unit test covers the separate construction paths and their event-class
allow lists. Existing fixture provenance, policy, schema, encryption, and
parent-event tests continue to pass.

## Local verification on merged main

Target: MacBook Pro `MacBookPro17,1`, Apple M1, 8 cores, 8 GB; macOS `26.6.2`,
Darwin `25.6.0`, `arm64`; `rustc 1.88.0`, Cargo `1.88.0`, Python `3.9.6`.

- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, 1 origin unit test, 21 Rust
  integration tests, clippy, and locked all-target tests;
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the denial canary and full product suite;
- `cargo +1.88.0 doc --no-deps` — pass;
- `python3 scripts/roadmap.py check` — pass: 160 tasks, 12 milestones, 488
  dependency edges;
- `python3 scripts/fixture-manifest.py check` and `python3 scripts/reproducibility.py check` — pass;
- `git diff --check` — pass.

Hosted checks on PR #183 were green and served only as the protected-branch
merge gate. The target-local merged-main reproduction above is the acceptance
evidence.
