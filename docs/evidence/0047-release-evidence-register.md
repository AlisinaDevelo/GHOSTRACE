# Task 0047 evidence: release evidence register

Status: complete.

Task 0047 turns the five program outcomes into a fail-closed release contract.
The register covers all twelve milestones and 36 exit measures. Each measure has
a quantified or binary target, a repository-relative proof artifact, required
scope, freshness window, and one of `planned`, `observed`, `inferred`, or
`unavailable` evidence states.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0047-CODE-F304262` — `f3042629bd89105042e122845ed7e6da8eca03d1` |
| Pull request | [#177](https://github.com/AlisinaDevelo/GHOSTRACE/pull/177) |
| Protected-main merge | `GHOSTRACE-0047-MERGED-AE90AA2` — `ae90aa28425a40753ea385f710545c5df5ab2582` |
| Post-merge verification log | `GHOSTRACE-0047-POSTMERGE-LOG-8031C1` — SHA-256 `8031c1224d79c295a543d7b974c836ff515299f2bec9f7b26490738d36de748e` |

The log is retained locally only; the public report records its digest and
redacts serial numbers, UUIDs, usernames, and computer names.

## Acceptance mapping

1. **Every milestone has measurable exit measures and proof artifacts.**
   `planning/release-evidence-register.json` contains M0 through M11, 36
   measures, explicit `kind`/`target` values, and stable `docs/evidence/...`
   artifact references. M6 performance thresholds reproduce the published task
   0039 CPU, memory, throughput, latency, RSS, and WAL limits.
2. **Evidence states are distinct.** The register defines and validates
   `planned`, `observed`, `inferred`, and `unavailable`. The initial snapshot
   reports all 36 measures as `planned`; no future release claim is implied.
3. **Missing, stale, narrow, or non-observed evidence blocks.**
   `scripts/release-evidence.py gate` requires `observed`, existing artifacts,
   `observed_at <= as_of`, freshness-window validity, and scope coverage. It
   returns structured blockers and a non-zero status for planned, inferred,
   unavailable, stale, future, missing, or narrow-scope evidence.

## Verification environment

- Device: MacBook Pro `MacBookPro17,1`, Apple M1, 8 cores, 8 GB RAM.
- OS: macOS `26.6.2 (25G83)`, Darwin `25.6.0`, `arm64`.
- Toolchain: `rustc 1.88.0 (6b00bc388 2025-06-23)`, Cargo `1.88.0
  (873a06493 2025-05-10)`, host `aarch64-apple-darwin`.
- Exact source under test: protected-main SHA
  `ae90aa28425a40753ea385f710545c5df5ab2582`.

The inventory intentionally records no serial number, hardware UUID, user
name, or computer name.

## Test matrix

The post-merge log `GHOSTRACE-0047-POSTMERGE-LOG-8031C1` records the exact
merged-SHA rerun:

- `python3 scripts/release-evidence.py check` — pass: 12 milestones, 36
  measures; state counts are planned=36 and observed/inferred/unavailable=0.
- `python3 scripts/release-evidence.py gate --milestone M0 --as-of 2026-08-24`
  — intentionally blocked with exit 1 and three `state_planned` blockers. This
  is the required fail-closed behavior, not a failed release claim.
- `python3 -m unittest discover -s tests -p 'test_*.py'` — pass: 28 tests,
  including five release-register tests for all four states, planned/inferred/
  unavailable blocking, stale evidence, and narrow scope.
- `python3 scripts/roadmap.py check` — pass: 160 tasks, 12 milestones,
  488 dependency edges.
- `python3 -m json.tool planning/release-evidence-register.json` and
  `python3 -m py_compile scripts/release-evidence.py tests/test_release_evidence.py`
  — pass.
- `cargo +1.88.0 fmt --all -- --check` — pass.
- `cargo +1.88.0 test --locked --all-targets --all-features` — pass: 20
  vertical-slice tests, 1 privacy regression, 5 support-matrix tests, and the
  ignored network canary outside the denial lane.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  — pass.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec` with
  `GHOSTRACE_OFFLINE_ENFORCED=1`: canary, privacy fixture,
  explanation/export, and complete offline suite passed with
  `CARGO_NET_OFFLINE=true`.
- `git diff --check` — pass.

GitHub Actions checks for PR #177 were green and protected main accepted the
merge, but Actions are not acceptance evidence for this task. Local
`cargo-deny` and `cargo-audit` binaries were unavailable on this Mac; hosted
policy/advisory gates passed. The register intentionally leaves future release
measures planned until their target evidence exists.

## Gate limitations

The register is a release-control mechanism, not a claim that later milestones
are complete. A fixture-only run cannot close a live-collector gate; a Linux or
simulator result cannot close a named macOS target; and an inferred or
unavailable result remains a blocker. Each future observed entry must retain the
source revision, target, toolchain, workload/corpus digest, artifact digest,
limitations, and expiry in its evidence report.
