# Task 0019 evidence: deterministic evidence-backed explain

Status: implementation, review, merge, and protected-main device reproduction
complete.

The parent capability is composed of three reviewed implementation PRs:
[claim grammar #283](https://github.com/AlisinaDevelo/GHOSTRACE/pull/283),
[correlation registry #285](https://github.com/AlisinaDevelo/GHOSTRACE/pull/285),
and [determinism/counterexamples #287](https://github.com/AlisinaDevelo/GHOSTRACE/pull/287).
They are present on protected `main`; their merged implementation SHAs are
`aa01b23985a7a47da86cd1d2464cc35cac2a4c29`,
`fe01b7104eda0ab339dcb83e7ddd87ae127b39b8`, and
`4d96efc47d707873ddca0dab956082379f4817d1`. This parent evidence change is
tracked by PR #298 and is the retained acceptance record for issue #23.

## Contract and acceptance mapping

`explain` walks a bounded parent chain, rejects cycles, preserves each event's
evidence level, renders through the versioned claim grammar, cites event UUIDs,
and includes explicit coverage gaps, source errors, and temporal ambiguity
warnings. Correlation identity carries the registry and rule versions. The
implementation is ordinary Rust/library code with no LLM or network boundary;
the offline pipe is a negative proof of that property.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Every statement cites event IDs. | `ExplanationStatement` carries `event_id`, `parent_event_id`, and `citations`; `render_claim` emits the cited UUID and the vertical-slice/claim-grammar tests require every statement citation to equal its source event ID. | PASS |
| Direct and inferred facts are labeled. | `Evidence` is preserved in `RenderedClaim` and `ExplanationStatement`; the claim renderer emits distinct Direct, Contextual, Inferred, and Unknown labels. `tests/claim_grammar.rs` and `tests/explanation_determinism.rs` exercise all four levels and the correlation rule only emits `Evidence::Inferred` for bounded authorized inputs. | PASS |
| Gaps and coverage are shown. | `CoverageSummary` counts chain gaps and emits gap/source-error/temporal-ambiguity warnings; claim descriptors distinguish explicit status from limit-of-interpretation behavior. The fixture explanation, gap grammar, correlation unknown-coverage cases, and vertical slice assert visible gap evidence. | PASS |
| Identical input produces identical output. | `explanation_identity` binds ordered event IDs, policy identity/version, and correlation versions. Golden, permutation, equal-timestamp, irrelevant-event, page-boundary, and mutation tests require identical serialized claims, identities, statements, and cited IDs. | PASS |
| Explanation has no LLM dependency. | The explanation path is deterministic Rust code over journal metadata and checked-in templates; no model, network, prompt, or runtime service is used. The network-denied canary, privacy suite, reproducibility pipe, and static dependency/audit gates pass on the target device. | PASS |

The deterministic renderer intentionally does not claim intent, completeness,
process attribution, unsupported causality, or old-to-new rename identity. An
inferred correlation is an evidence transformation with explicit rule bounds,
not a causal conclusion.

## Protected-main device reproduction

The focused parent matrix and all safety pipes below ran on protected `main`
`1fea9a1967cfd8b9e8033e2a07719d03a6bf762d` on the named device. Combined
stdout and stderr are retained in `/tmp` and hashed here.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Counterexample fixture | `fixtures/explanation-counterexamples-v1.json`, 2115 bytes, SHA-256 `7b2158d83919ef03649c54bedbd5bed2b2f259f1e05c9b6e7b8de377298b7d49` |
| Causal fixture | `fixtures/causal-chain.jsonl`, 4740 bytes, SHA-256 `22c4e9df8520cbd5b14b343ea2f6c5cca3bcdf0de34433384f6e069b7984bdc8` |
| Source | protected `main` `1fea9a1967cfd8b9e8033e2a07719d03a6bf762d` |
| Date | 2026-08-26 |

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| focused debug: claim grammar, correlation, determinism, vertical slice | 4 + 4 + 4 + 28 passed, exit 0 | `da7e9dce56a5d4a3676fab8949da1468b3d29f8cdc65ddea367b1b3447a6659f` |
| focused release: claim grammar, correlation, determinism, vertical slice | 4 + 4 + 4 + 28 passed, exit 0 | `ab24775e3e35d6e73f704865a8f11bdba91b4d1e59a125585e4f177237e855de` |
| `scripts/reproducibility-test.sh` (final rerun) | exit 0; all checks passed | `4b1006ffe3ad06f1dab0d5b1ccbd456a5df127462fbc330b7a9c82b63d694ba7` |
| `scripts/offline-network-test.sh` | exit 0 under macOS `sandbox-exec` | `b18304f6e9692609d1dbb931709ec36677274aeb9cf7e03dd501b9173200acb0` |
| `scripts/fsevents-sanitizer.sh` | exit 0; native test passed with documented suppressions | `5142f783eec2ecc330236f6aab6ee880533b839e22acc8836d44d9de13e2d95b` |
| isolated native safe-storm retry | exit 0; 170 observations, 0 ordering inversions, 3/3 recoveries, 0 drops | `86bbf6d5999878d106cf24f6a41597572e574067c1d8071e786389d89bf485ce` |

The first full reproducibility attempt surfaced a transient native
`CursorRegression { event_source: Filesystem }` in the lifecycle corpus (exit
101; receipt `fa31c9180698c1b9254e97e9394bef566c7870d2863f6ffed16b36f7ef57525f`).
The lifecycle test was immediately rerun in isolation and passed with the
receipt above; a second complete reproducibility run then passed all checks
with receipt `4b1006ffe3ad06f1dab0d5b1ccbd456a5df127462fbc330b7a9c82b63d694ba7`.
The event-ordering guard remains fail-closed; this transient observation is
retained rather than hidden.

The reproducibility lane includes schema/fixture checks, deterministic CLI
explanation/export comparisons, capture refusal, Python tests, Clippy, and all
Rust targets. The offline lane includes the denial canary, privacy regression,
and complete product suite. The sanitizer lane exercised the native lifecycle
integration test and reported no sanitizer finding.

### MVP explanation receipt

The post-merge MVP path runs the deterministic parent-chain explanation test
with resource accounting:

```text
cargo +1.88.0 test --locked --test vertical_slice \
  fixture_explanation_is_deterministic_and_cites_the_parent_chain \
  -- --exact --nocapture
```

It exited 0 with 1 passed and 27 filtered tests. The combined receipt is
`/tmp/ghostrace-0019-merged-mvp.log`, SHA-256
`4c8107d394c1c978b5b6b7e24c5cb7e2d87459a75bfb10d94b2a0e4647256c60`; elapsed
time was 0.31 s and maximum resident set size was 59,703,296 bytes. The
receipt contains only synthetic fixture test output and resource counters.

## Existing child evidence and hosted review

The child records retain their own implementation and protected-main receipts:

- [0080 bounded evidence-claim grammar](0080-bounded-evidence-claim-grammar.md)
  covers all 12 templates, required facts, prohibited implications, locales,
  evidence labels, and gap behavior.
- [0081 versioned correlation registry](0081-versioned-correlation-rule-registry.md)
  covers policy-bounded inferred evidence, positive/negative/ambiguous/adversarial/
  clock-skew fixtures, and identity versioning.
- [0082 explanation determinism](0082-explanation-determinism.md) covers golden,
  permutation, equal-time, irrelevant-event, page-boundary, and mutation
  properties.

All three implementation PRs passed protected repository checks before merge.
The current main checkout also passes the hosted gates on the docs-only parent
record; the device receipts above remain the acceptance evidence rather than a
CI substitution.

The parent closure validators also passed: 46 Python tests, the roadmap
validator with 160 tasks, 12 milestones, 488 dependency edges, 108 parent
edges, and one additional done task, and the release-evidence validator with 36
measures. These checks were rerun after the protected-main reproduction.

| Closure validator | Result | Log SHA-256 |
| --- | --- | --- |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | 46 passed, exit 0 | `5959fd127f3e4ca1a241752487e7ec6b45d0f09ad2c81e63b304ab9eb7cda0b7` |
| `python3 scripts/roadmap.py check` | 160 tasks, 12 milestones, 488 dependency edges, 108 parent edges, exit 0 | `87071c70cc9899ab088e15a71cb913cb0916a3a4bde2b80d790ddc88106c2793` |
| `python3 scripts/release-evidence.py check` | 36 measures, 12 milestones, exit 0 | `82ff0862f41c422cc4ad89be1d9fb40f64e2fda7179a690ffadff32994edc3f7` |

## Boundaries and limitations

- The explanation is parent-chain scoped. It reports gaps and source errors in
  that chain and does not invent a whole-journal completeness claim for events
  outside the requested chain.
- Correlation is bounded to the versioned rule registry and authorized event
  metadata; unknown coverage, denied policy, equal times, and clock rollback
  abstain instead of becoming positive evidence.
- Fixtures and pipes are synthetic, offline, and path-free. No user paths,
  account names, credentials, payload contents, token plaintext, or live
  ambient capture are claimed. Sleep/wake, logout, volume detach, and live
  Keychain authorization remain explicit no-go/ignored rows.
- This parent task does not close the M3 aggregate gate or claim production
  causal reconstruction, collector completeness, or release readiness.
