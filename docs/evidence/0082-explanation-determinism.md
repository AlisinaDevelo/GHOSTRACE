# Task 0082 evidence: explanation determinism and counterexamples

The explanation renderer now has a manifest-bound golden, permutation, and
mutation contract. Implementation PR [#287](https://github.com/AlisinaDevelo/GHOSTRACE/pull/287)
was merged to protected `main` at
`4d96efc47d707873ddca0dab956082379f4817d1` on 2026-08-25. The reviewed
implementation commit before squash was
`6dd73a74f852f2ba69301fc71ff7127e67b0bc58`. Issue #86 is closed only after
this evidence change is merged and linked.

## Contract and acceptance mapping

[`fixtures/explanation-counterexamples-v1.json`](../../fixtures/explanation-counterexamples-v1.json)
is synthetic-only, offline, and manifest-bound (SHA-256
`7b2158d83919ef03649c54bedbd5bed2b2f259f1e05c9b6e7b8de377298b7d49`, 2,115
bytes). The focused suite is
[`tests/explanation_determinism.rs`](../../tests/explanation_determinism.rs).

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Golden cases cover every claim template, evidence level, gap interaction, and conflict outcome. | `golden_matrix_covers_every_template_evidence_gap_and_conflict_outcome` checks all 12 `ClaimTemplateId` values, direct/contextual/inferred/unknown evidence, no-gap and recorded-gap states, explicit-status outcomes for coverage/policy/source errors, and downgraded unknown evidence. It serializes each matrix cell twice and requires identical bytes, cited event IDs, and no forbidden intent/causality wording. | PASS |
| Property tests permute ingestion, equal timestamps, irrelevant events, and page boundaries without changing supported claims. | `property_permutations_equal_times_irrelevant_events_and_page_boundaries_preserve_claims` checks all 24 permutations of four independent observations, eight equal-timestamp permutations, page sizes 1/2/3/5/256, and a filtered filesystem projection with irrelevant events present/absent. `explanation_bytes_and_identity_are_stable_with_irrelevant_events` also keeps the causal parent-chain bytes, identity, IDs, and statements stable after an unrelated event is ingested. | PASS |
| Mutation tests demonstrate that removing a required observation downgrades or removes the claim. | `mutation_cases_remove_required_observations_and_downgrade_or_remove_claims` removes the second cross-source observation and requires `Evidence::Unknown` with `requires_distinct_sources`; it then removes the filesystem parent event, reparents its child, and requires the chain to shrink from eight to seven statements with the filesystem claim absent. | PASS |

The tests exercise only bounded event metadata and checked-in synthetic payloads.
They do not infer intent, completeness, process attribution, or causality, and
they do not enable a live collector.

## Protected-main device reproduction

The exact merged SHA was rerun on the named device. Every command below exited
0; the log digests are retained here so a later reproduction can detect drift.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Source | protected `main` `4d96efc47d707873ddca0dab956082379f4817d1` |
| Device receipt SHA-256 | `fb000ade38b7a0746e3f4749594f261ae749fa79ae480ef661fe2cded018ae8c` |
| Date | 2026-08-25 |

Commands and retained receipts:

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| `cargo +1.88.0 test --locked --test explanation_determinism -- --nocapture` | 4 passed, exit 0 | `ae7f2ace5b1e904e9946aca3d43dd7c629464302b83c71540c4a5af29cb2ae19` |
| `cargo +1.88.0 test --locked --all-targets --all-features` | all targets passed, exit 0 | `98b58e1b58aa2f85c8c02a9c43b80c0ca321ad30021b5d51bcff446bd1aa101f` |
| `cargo +1.88.0 test --locked --all-targets --all-features --release` | all targets passed, exit 0 | `d2ccbbb3373677efb6f1f8dbbe9b180c5668b7a6995807e369c2915a10fd07a6` |
| `cargo +1.88.0 fmt --all -- --check`, Clippy `-D warnings`, rustdoc, validators, 46 Python tests | all passed, exit 0 through reproducibility pipe | `74537248dacd4141555eec2dbe733ac255e5cb49148d7a1846b34a7e6e6c5680` |
| `scripts/reproducibility-test.sh` | all checks passed, exit 0 | `74537248dacd4141555eec2dbe733ac255e5cb49148d7a1846b34a7e6e6c5680` |
| `scripts/offline-network-test.sh` | denial canary, privacy, and complete suite passed, exit 0 | `bcbf186bc03b337e93316fd30684781eb7f5faa0abbb633eff2a623406b45d7e` |
| `scripts/fsevents-sanitizer.sh` | native test passed, no finding, exit 0 | `1ac22cd72061ce881161136526896f9db86b7a5dbb9220665a5a6e3b0f067b21` |
| `git diff --check` | clean, exit 0 | included in the reproducibility receipt |

The sanitizer output contains only its documented runtime-initialization
suppression count. The offline lane enforced `sandbox-exec` network denial
before the privacy and product suites. The complete pipe reported nine fixtures,
160 roadmap tasks, 12 milestones, 488 dependency edges, 46 Python tests, and
zero diff-check findings.

## Local implementation pipe and hosted merge

Before the protected-main merge, the same local reproducibility script passed on
the implementation tree (PR #287) with exit 0. Its retained log is
`/tmp/ghostrace-0082-premerge-repro.log`, SHA-256
`0b45216601da2e3284baecb1c05794c45ab11d47ccc9c2ced84251b98c2a227e`.

PR #287 had 19 successful hosted checks across duplicate push and pull-request
runs: rustfmt, Clippy, Linux stable, Linux MSRV, macOS stable, offline fixture,
Cargo policy, advisories, dependency review, and roadmap. The push run was
`32893905633`; the pull-request run was `32893883816`. No check was skipped or
failed before the protected-main merge.

## Boundaries and limitations

- The matrix is synthetic and offline; no user paths, accounts, credentials, or
  payload contents are retained or echoed.
- Equal timestamps and ingest order are determinism inputs, not causal evidence.
- The mutation proof is limited to the current correlation rule and parent-chain
  explanation; it does not claim all future rules are complete.
- The native sanitizer and safe FSEvents tests ran on this device. Sleep/wake,
  logout, volume detach, and live Keychain authorization remain explicit no-go or
  ignored rows and were not converted into passes.
- This task does not close the M3 aggregate gate or claim production collector,
  causal reconstruction, or release readiness.
