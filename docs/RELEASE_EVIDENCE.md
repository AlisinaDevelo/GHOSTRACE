# Release evidence register

GHOSTRACE closes a milestone only when its required evidence is current, observed
on the declared target, and scoped to the complete gate. Issue state, code volume,
a green narrow test, or an inferred result is not a release decision.

The normative register is [`planning/release-evidence-register.json`](../planning/release-evidence-register.json).
It is machine-readable so a release check can fail closed without interpreting a
narrative report. Each of the twelve milestones has one or more exit measures. Every
measure names:

- a binary, count, rate, or threshold target;
- the artifact path that proves the target;
- the required scope (milestone, surface, target, and corpus where relevant);
- a freshness window in days; and
- the current evidence state and observation metadata.

## Evidence states

| State | Meaning | Can close a gate? |
| --- | --- | --- |
| `planned` | Method and artifact are named, but the measurement has not run. | No |
| `observed` | The named artifact records the result at the required scope and date. | Yes, while fresh |
| `inferred` | The result is derived from other evidence. | No; retain as context only |
| `unavailable` | The target or measurement could not be run. | No; record the no-go or limitation |

The current register intentionally contains 36 `planned` measure entries. Existing
child evidence (for example the privacy corpus and offline lane) is not silently
promoted to an aggregate milestone pass; the aggregate report must demonstrate full
scope and currentness.

## Milestone exit map

The detailed thresholds and evidence records live in the JSON register. This map is
the human index of the required gate surfaces:

| Milestone | Exit measures | Primary evidence artifacts |
| --- | --- | --- |
| M0 | 13/13 M0 tasks evidenced; privacy/security boundaries pass on the minimum target; identity/platform/toolchain decisions recorded | `docs/evidence/M0-release-gate.md`, `M0-privacy-security.md`, `M0-foundation-decisions.md` |
| M1 | All retained fields/origins and consent paths covered; 100/100 injected failures recover or gap; no forbidden data across storage/export paths | `M1-typed-origin-consent.md`, `M1-storage-recovery.md`, `M1-privacy-regression.md` |
| M2 | All documented FSEvents flags mapped with zero containment escapes; all loss/restart cases gap explicitly; reproducible storm benchmark | `M2-fsevents-containment.md`, `M2-cursor-gap-recovery.md`, `M2-fsevents-benchmark.md` |
| M3 | Three replay outputs byte-identical; 100% claims cited and gap-aware; retention/deletion/integrity matrix complete | `M3-deterministic-query-explain-export.md`, `M3-claim-grammar.md`, `M3-retention-integrity.md` |
| M4 | Shell/Git/frontmost prohibited fields absent; ground-truth workflow evaluation complete; unavailable history becomes gaps | `M4-context-privacy-corpus.md`, `M4-workflow-evaluation.md`, `M4-history-gaps.md` |
| M5 | Hostile browser/native messages fail closed; service requires peer/capability checks with no TCP fallback; UI/lifecycle accessibility and rollback covered | `M5-browser-security.md`, `M5-local-service-boundary.md`, `M5-ui-lifecycle.md` |
| M6 | Independent universal builds match; all distributed artifacts have release-integrity records; task 0039 thresholds pass; red-team/incident evidence complete | `M6-reproducible-artifacts.md`, `M6-release-integrity.md`, `M6-performance-gates.md`, `M6-red-team-incident-readiness.md` |
| M7 | 100/100 backup/recovery cases behave as specified; every supported macOS major has annual evidence; support bundle is telemetry-free | `M7-backup-recovery.md`, `M7-annual-macos-validation.md`, `M7-telemetry-free-support.md` |
| M8 | Imported records retain external origin; all adapter capability/conformance cases pass; encrypted bundle refusal matrix complete | `M8-imported-evidence-trust.md`, `M8-adapter-conformance.md`, `M8-encrypted-bundles.md` |
| M9 | Three research reruns reproduce; every evaluation dimension reports uncertainty/abstention; negative findings and leakage/resource artifacts retained | `M9-reproducible-research.md`, `M9-evaluation-dimensions.md` |
| M10 | Admission/revocation cases explicit; isolation go/no-go observed on target or marked unavailable; security-response drill complete | `M10-admission-revocation.md`, `M10-isolation-decision.md`, `M10-security-response.md` |
| M11 | 100/100 migration/rollback cases pass; independent audit and release-candidate recovery complete; v2/LTS/2032 decision signed | `M11-migration-compaction.md`, `M11-independent-audit-rc.md`, `M11-lts-sustainability.md` |

Artifact names are stable register references, not claims that the files already
exist. An entry becomes `observed` only after the artifact is written, its digest,
source revision, target, toolchain, scope, limitations, and observation date are
recorded in the report.

## Gate algorithm

The local checker is [`scripts/release-evidence.py`](../scripts/release-evidence.py):

```sh
python3 scripts/release-evidence.py check
python3 scripts/release-evidence.py gate --milestone M0 --as-of 2026-08-24
```

`check` validates that all milestones in `planning/program.json` are represented,
that every measure has a target, artifact, scope, and positive freshness window, and
that the four evidence states are preserved. `gate` returns a non-zero status and
structured blockers when any measure is:

1. not `observed`;
2. observed after the gate date or older than its freshness window;
3. missing its named repository artifact; or
4. narrower in scope than the required measure.

The checker requires `observed_at <= as_of`, checks
`observed_at + freshness_days >= as_of`, and requires the evidence scope to contain
every required scope token. It never downgrades a missing or unavailable target to a
pass. A future release may replace a planned entry with an observed report, but must
retain the prior state and limitation in the evidence history rather than deleting
it.

## Ownership and review

The task ledger and register are reviewed together. A release owner must confirm the
exact source revision, target device/OS/architecture, toolchain and locked inputs,
artifact digests, happy and negative tests, privacy scans, resource results, failure
and recovery outcomes, limitations, and expiry. Evidence scoped to Linux, a simulator,
CI, a fixture-only path, or a narrower collector cannot close a macOS production gate
unless the measure explicitly names that narrower scope.

When a target is unavailable, the register records `unavailable`, the reason, owner,
and no-go decision. It does not substitute a hosted runner or inferred result. This
keeps the program honest about the difference between a plan, an observation, a
derived interpretation, and an unmeasured surface.
