# Task 0081 evidence: versioned cross-source correlation rule registry

Implementation, hosted review, protected-main merge, and device reproduction
are complete for the bounded correlation-rule registry.

Implementation PR [#285](https://github.com/AlisinaDevelo/GHOSTRACE/pull/285)
was merged to protected `main` at
`fe01b7104eda0ab339dcb83e7ddd87ae127b39b8` on 2026-08-25. The reviewed
implementation commit before squash was `4fc30dc`. The protected-main commit
is the acceptance source for this record. Issue #85 is closed only after this
evidence change is merged and linked.

## Contract and acceptance mapping

The registry in `src/correlation.rs` is version `1` and currently contains the
`cross_source_temporal_adjacency` rule at version `1`. Its descriptor is the
source of truth for permitted input fields, exclusions, output evidence, the
60-second/256-event bounds, and the five counterexample classes. A
`CorrelationQuery` binds policy profile identity/version, scope digest, source
set, time window, and maximum input count. The evaluator authorizes every event
against that policy before reading bounded event metadata.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Rules cannot read fields outside the query policy or convert an unknown source interval into positive evidence. | `CorrelationQuery::validate` binds profile/version, scope digest, enabled sources, time window, and input bound. `evaluate` gates every event through the policy before reading it, rejects denied scope, unknown evidence, gap/status markers, unsupported kinds, equal timestamps, and clock rollback, and emits `Evidence::Inferred` only for two distinct authorized observations inside the bound. `tests/correlation_rules.rs::positive_and_negative_results_are_bounded_and_never_upgrade_unknown_scope` and `::unknown_intervals_evidence_and_clock_skew_abstain` exercise positive, denied, unknown, and clock-skew paths. | PASS |
| Each rule includes positive, negative, ambiguous, adversarial, and clock-skew fixtures. | `fixtures/correlation-rules-v1.json` is synthetic-only, network-free, and manifest-bound. It names one rule and exactly five fixture classes: positive cross-source observations, negative same-source observations, ambiguous unknown evidence, adversarial policy scope, and clock-skew rollback. `tests/correlation_rules.rs::registry_is_inspectable_and_fixture_manifest_covers_every_counterexample_class` checks the descriptor and manifest together. | PASS |
| Changing a rule version changes explanation identity and remains reproducible for historical exports. | `explanation_identity_for_rule_version` includes rule and registry versions in the deterministic digest; the correlation test proves a changed rule version changes the identity. `Explanation` carries both versions and the identity, while `ExportManifest` records both versions for historical JSONL output. The focused debug/release tests and reproducibility pipe repeat the same fixture/export paths. | PASS |

The implementation intentionally reports a bounded inferred adjacency, not
causality, intent, completeness, process attribution, or a claim about an
unknown interval. Event IDs, source, kind, observed time, evidence level,
policy scope, and coverage markers are the only descriptor-listed inputs.

## Protected-main device reproduction

The focused rule/claim/vertical-slice tests, all-target suites, validators,
reproducibility pipe, network-denied suite, and native sanitizer were rerun
from protected `main` `fe01b7104eda0ab339dcb83e7ddd87ae127b39b8` on the named
device. Every command exited 0.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Source | protected `main` `fe01b7104eda0ab339dcb83e7ddd87ae127b39b8` |
| Date | 2026-08-25 |

Commands:

```text
cargo +1.88.0 test --locked --test correlation_rules --test claim_grammar --test vertical_slice -- --nocapture
cargo +1.88.0 test --locked --release --test correlation_rules --test claim_grammar --test vertical_slice -- --nocapture
cargo +1.88.0 test --locked --all-targets --all-features
cargo +1.88.0 test --locked --all-targets --all-features --release
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.88.0 doc --locked --all-features --no-deps
python3 scripts/fixture-manifest.py check
python3 scripts/roadmap.py check
python3 scripts/release-evidence.py check
python3 scripts/identity-audit.py check
python3 -m unittest discover -s tests -p 'test_*.py'
./scripts/reproducibility-test.sh
scripts/offline-network-test.sh
scripts/fsevents-sanitizer.sh
git diff --check
```

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| protected-main focused debug (correlation, claim grammar, vertical slice) | 4 + 4 + 28 passed, exit 0 | `2a100ff375f2a5c32cf68d145ababa1c5e42b5698808a91ef4eb7b2c69c53067` |
| protected-main focused release (correlation, claim grammar, vertical slice) | 4 + 4 + 28 passed, exit 0 | `beead44d097def29563837e36958755beb36e9094efab4ec3c1e659f99418ecd` |
| protected-main all-target/all-feature debug | all targets passed, exit 0 | `4def3d71b596677446636c717950b095991b7421215eccdfa3e15b26a480c9f7` |
| protected-main all-target/all-feature release | all targets passed, exit 0 | `0964a05c9fe079d1a47865dd7c06cf0b48521c3a9392bdde42d82b8eaf897fca` |
| protected-main contracts (fmt, clippy, rustdoc, fixture/roadmap/evidence/identity validators, 46 Python tests, diff check) | all passed, exit 0 | `ce32831bfc1dd96a89fb73c649fb4991d82ac6728289d7f00266b141d33bac7c` |
| protected-main reproducibility | all checks passed, exit 0 | `b85c13326f40776a5fb6df230b1411264f98699e2b1d8b2be28f383fb86b114d` |
| protected-main network-denied product pipe | canary, privacy, and complete suite passed, exit 0 | `649cbb10abafd59bc4cf8d7b9ee28f1b99caa88158b26972e6b9df3abb86ef9b` |
| protected-main FSEvents sanitizer | native test passed; 3 documented suppressions; no finding; exit 0 | `f703ef063a00560c84ab5628c2ffd073ad66058aece17a276a004e5eadcd87e2` |

The protected-main contract report recorded 8 fixtures, 160 roadmap tasks,
12 milestones, 488 dependency edges, 46 Python tests, and zero diff-check
findings. The reproducibility pipe reported `reproducibility: all checks
passed`. The offline lane denied network access before running the privacy and
product suites. The native sanitizer completed with its documented runtime
initialization suppressions and no sanitizer finding.

## Local implementation pipe

Before merge, the implementation branch ran the same device-local matrix.
Every required lane exited 0.

| Lane | Log SHA-256 |
| --- | --- |
| all-target/all-feature debug Rust tests | `0d79eedb7b268f6e67d87d22514974af745ee9dc42e56ea4fbf732cee0e0fef1` |
| all-target/all-feature release Rust tests | `d662c9a11a1b41bbda79bfbd02416a886bed5780698dd9a3c86cece25b621b74` |
| focused debug rule/claim/vertical tests | `c60ea903bb46a33866e7f881e828fff3f7ac01bb8b0562dfbb8fe417ae924fa0` |
| focused release rule/claim/vertical tests | `2365a2edf52909ffe3eb4dd1bed1f3ebdbba7158b1ed9f154e59632000a569b6` |
| fmt, clippy, rustdoc, Python, fixture/roadmap/evidence/identity validators | `20cc40314938e7a2c705d5670a3c5f454a746bb0cc01c4a52dfc088629d6d0a2` |
| reproducibility contract check | `814633d5543091cf77a2e9eef1d57a86c149da62792274b2af51456c90a566c4` |
| offline network-denied product pipe | `2fa0c4c7260154d43dc42cb6b169dd944299edf07e54beffb51173e50909d3b0` |
| FSEvents sanitizer lane | `6a8a92474ec3959747b1840fd315150d756cd0ebf68ee4d48f64a1993acbe47e` |

## Hosted review and merge

PR #285 passed duplicate hosted runs for rustfmt, Clippy, Linux stable, Linux
MSRV, macOS stable, offline fixture, Cargo policy, advisories, dependency
review, and roadmap. Both workflow runs completed successfully before the
protected-main merge. Hosted checks were review gates; the device pipe and
post-merge device matrix above are the acceptance evidence.

## Boundaries and limitations

- Registry version `1` currently contains one rule and intentionally bounds
  evaluation to a 60-second window and 256 input events.
- The rule emits `inferred` only for two distinct authorized direct or
  contextual observations. It makes no causal, intent, completeness, process,
  or historical-capture claim.
- Fixtures are synthetic, offline, and network-free. No user paths, payload
  contents, account names, or token plaintext are retained or echoed.
- Unknown evidence, unknown coverage, policy denial, equal observed times,
  unsupported event kinds, and source clock rollback remain explicit unknown
  outcomes rather than positive evidence.
- No interactive sleep, logout, volume detach, or live Keychain lifecycle was
  performed; the Keychain lifecycle test remains explicitly ignored because it
  requires separate device authorization.
- This task does not close the M3 aggregate gate or claim native collector
  completeness, causal reconstruction, or release readiness.
