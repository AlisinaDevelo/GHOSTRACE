# GHOSTRACE 2026–2031 program

This engineering and research program runs from 2026-08-23 through
2031-12-31. It defines 160 planning tasks across 12 milestones. Dates
are planning boundaries, not promises. A milestone closes only when its acceptance
evidence is current; issue closure, code volume, or a green narrow test is not a
substitute.

GHOSTRACE remains a local macOS causal event journal with explicit consent,
minimized metadata, evidence-linked explanations, and first-class gaps. The program
does not authorize employee monitoring, content capture, silent upload, remote
control, legal chain-of-custody claims, or causal conclusions unsupported by the
recorded sources.

## Program outcomes

The program is successful only if it improves all five outcomes together:

1. **Privacy control.** A person can see, grant, narrow, pause, revoke, export, and
   delete each source scope. Forbidden-data sentinels stay absent from journals,
   sidecars, diagnostics, exports, support artifacts, and failure paths.
2. **Evidence honesty.** Every claim cites observations and evidence level. Empty
   results carry coverage context. Dropped, denied, unavailable, retained-out, and
   unknown intervals remain visible. Controlled evaluation penalizes unsupported
   claims and rewards correct abstention.
3. **Durable local operation.** Keys, database state, cursors, policy history, gaps,
   upgrades, backups, recovery, and integrity survive the failures each release
   says it supports, without a cloud dependency.
4. **Bounded cost.** Ingest, queues, WAL growth, queries, explanations, retention,
   export, UI, and background operation stay within published memory, CPU, energy,
   disk, latency, and recovery budgets on named hardware and macOS versions.
5. **Verifiable delivery.** Releases bind source, locked inputs, SBOM, build
   provenance, signatures, entitlements, notarization, compatibility, privacy,
   performance, and incident evidence to the exact distributed artifacts.

Task 0047 defines the [release evidence register](RELEASE_EVIDENCE.md) for each
target, artifact, scope, date, result, limitation, and owner. Exit thresholds remain
planned until the register records fresh observed evidence at the complete gate
scope. Unknown or unavailable evidence remains unknown; it cannot be converted into
a pass.

## Milestones

| Milestone | Due | Tasks | Program exit |
| --- | --- | ---: | --- |
| M0 | 2026-09-30 | 13 | Fixture-only public contract, versioned event envelope, identity decision, executable privacy boundaries, supported-platform matrix, and reproducible planning |
| M1 | 2027-03-31 | 20 | Typed origins and fields, revocable consent, Keychain lifecycle, hardened SQLite WAL, bounded writer, and crash recovery |
| M2 | 2027-09-30 | 19 | Selected-root FSEvents with containment, volume-aware cursors, explicit loss, lifecycle recovery, and reproducible benchmarks |
| M3 | 2028-03-31 | 20 | Snapshot queries, evidence-claim grammar, deterministic explanation, streaming export, retention, deletion limits, and integrity |
| M4 | 2028-09-30 | 15 | Explicit shell and Git sources, frontmost context, secret-leakage tests, and multi-source workflow evaluation |
| M5 | 2029-03-31 | 23 | Browser security and pairing, constrained adapters, authenticated local service, accessible UI, and reversible launchd lifecycle |
| M6 | 2029-08-23 | 14 | v1 release evidence: reproducible universal builds, SBOM and SLSA provenance, notarization, performance, compatibility, red-team, and incident drills |
| M7 | 2030-02-28 | 7 | v1.1 operational resilience: diagnostics, backup and recovery, annual macOS validation, accessibility, distribution, and telemetry-free support |
| M8 | 2030-08-31 | 7 | Imported-evidence trust, W3C PROV, bounded offline interoperability, adapter capabilities and conformance, and encrypted bundles |
| M9 | 2031-02-28 | 7 | Reproducible research on causal precision, coverage, abstention, gap comprehension, privacy leakage, and longitudinal resource cost |
| M10 | 2031-08-31 | 7 | Governed ecosystem with stable adapter contracts, isolation decision, code admission, revocation, conformance evidence, and security response |
| M11 | 2031-12-31 | 8 | v2 and LTS decision, format and key migration, verified compaction, independent audit, release-candidate recovery, and 2032 sustainability plan |

The complete task graph is in [the Forge ledger](../.forge/tasks/README.md).
GitHub mirrors each task as an issue with the same stable ID, milestone, status,
workstream, priority, risks, native parent relationship, and native blocked-by edges.

## Workstreams

### Foundation, privacy, and storage

M0 and M1 turn the negative product contract into executable invariants. The work
includes semantic field wrappers, origin capabilities, prohibited-data tests,
network-denied execution, policy migrations, revocable consent, bounded refusal
reasons, macOS data-protection Keychain behavior, key rotation, no-follow database
creation, WAL sidecar policy, migration checksums, cursor monotonicity, and fault
injection.

Live collection remains refused until the capstone issues for policy, keys, storage,
writer, and recovery close on their child evidence. A completed fixture demo does
not satisfy those production gates.

### Filesystem evidence

M2 treats FSEvents as a lossy change-notification source. The collector maps every
documented flag, binds cursors to the chosen stream mode plus device and volume
identity, records dropped or wrapped history as gaps, tests APFS and path-containment
edge cases, prevents feedback loops, and publishes controlled storm and lifecycle
measurements. Task 0075 supplies the versioned storm/lifecycle corpus and a native
macOS receipt for private safe rows; sleep/wake, logout, and volume-detach remain
explicit guarded no-go rows until an authorized interactive device run. It does not
claim process attribution.

### Query, explanation, export, and integrity

M3 defines stable pagination and ordering, gap-aware windows, a bounded claim
grammar, versioned correlation rules, counterexample tests, an export schema
registry, streaming atomic plaintext export, redaction preview, retention dry-run,
deletion-residue limits, authenticated journal state, repair checkpoints, and an
optional Parquet profile. Explanations may abstain; they may not fill a gap with a
plausible story.

### Explicit developer and application context

M4 adds only integrations the person deliberately enables. The shell wrapper cannot
retain arguments, environment, input, or output. Git snapshots exclude content,
messages, remote credentials, and raw paths. Frontmost-application evidence excludes
titles and documents and remains contextual rather than actor attribution. A
ground-truth workflow corpus evaluates the combined claims.

### Browser, local service, and UI

M5 begins with a browser threat corpus and security ADR. Native messaging uses exact
extension identities, explicit pairing, bounded framing, replay protection, strict
origin minimization, and private-context refusal. The local service uses a restrictive
Unix socket with peer and capability checks and no TCP fallback. The read-only UI
must make evidence levels and gaps accessible. launchd operation remains per-user,
reversible, and consent-preserving.

### Release and operational resilience

M6 and M7 bind the exact artifacts to entitlements, reproducible-build evidence,
SBOM, SLSA provenance, Developer ID signing, hardened runtime, notarization,
compatibility, resource budgets, privacy red-team results, and incident drills. Day-two
work then adds redacted diagnostics, SQLite-safe backup and restore, annual macOS
testing, accessibility and localization, explicit update-channel decisions, and
telemetry-free support bundles.

### Interoperability and research

M8 permits only explicit offline exchange. Imported records retain an external origin
and cannot become native direct evidence. W3C PROV and OpenTelemetry profiles are
conservative mappings, not new collection channels. Adapter manifests declare every
permission, field, bound, cursor, gap, and network capability and must pass a
conformance harness.

M9 publishes synthetic ground truth and reproducible evaluation. It measures causal
claim precision, coverage, abstention, uncertainty comprehension, artifact-level
privacy leakage, and longitudinal energy and storage cost. Negative results and
counterexamples remain part of the public artifact.

### Ecosystem, v2, and long-term support

M10 does not assume third-party plug-ins are safe. It first stabilizes an adapter
contract, evaluates process or WASI-style isolation on macOS, binds code to capability
and conformance evidence, defines revocation, and scales governance and response. A
cross-platform study creates no porting promise.

M11 protects long-lived local evidence through an evidence-driven format-v2 decision,
cryptographic agility, verified compaction, deprecation and LTS policy, independent
audit, full migration and rollback release candidate, and an explicit continuation or
orderly-retirement decision for 2032.

## Sequencing and control rules

- A task is `ready` only when every `depends_on` task is verified `done`.
- A capstone issue is blocked by its granular child deliverables; later milestones
  depend on capstones rather than flattening hundreds of edges into one release issue.
- New data fields, collectors, permissions, network surfaces, import formats, or
  executable adapters require privacy, threat, schema, bounds, failure, migration,
  and regression evidence in the same milestone.
- A source that loses history emits a gap before resuming. A failed permission,
  policy, key, integrity, or compatibility gate refuses rather than degrading
  silently.
- Optional work may be rejected at its go/no-go task without weakening the baseline.
  This applies to Endpoint Security, Safari, telemetry import, automatic updates,
  plug-in execution, and cross-platform expansion.
- The identity collision with the unrelated VUSec GhostRace project must close at
  task 0041 before broad packaging or namespace publication.
- Scope is reviewed at every milestone. Tasks may be split when evidence shows a
  unit is not independently verifiable, but stable IDs are never recycled.

## Planning and verification workflow

The versioned `.forge/tasks` files are the task source of truth. Run:

```sh
python3 scripts/roadmap.py check
python3 scripts/roadmap.py index --write
```

Issue titles are the stable public identity for roadmap tasks. Public bodies contain
the goal, acceptance criteria, context, and notes, but never Forge markers or the
internal `Assigned:`, `Depends on:`, or `Parent:` routing lines. The publisher removes
those lines idempotently and verifies the resulting body before any other metadata
write. Native relationships and task state remain Forge-owned. Milestones, labels,
assignments, and public-body hygiene are reconciled from `planning/program.json` only
after an inspected Forge plan has zero pending native operations. This maintainer-only
workflow requires authenticated `gh`, `jq`, and the Forge task-ledger script. Set
`GHOSTRACE_FORGE_TASKS` to that script's absolute path, then run:

```sh
GHOSTRACE_TASK_TREE_DIGEST=$(python3 scripts/roadmap.py task-digest)
python3 "$GHOSTRACE_FORGE_TASKS" \
  --repo AlisinaDevelo/GHOSTRACE --tasks-dir .forge/tasks --json plan \
  | jq --arg repository 'AlisinaDevelo/GHOSTRACE' \
    --arg task_tree_digest "$GHOSTRACE_TASK_TREE_DIGEST" \
    '. + {repository: $repository, task_tree_digest: $task_tree_digest}' \
  > /tmp/ghostrace-forge-plan.json
jq -e '.authority == "local" and (.operations | length == 0)' \
  /tmp/ghostrace-forge-plan.json
python3 scripts/roadmap.py github-plan \
  --forge-plan /tmp/ghostrace-forge-plan.json \
  > /tmp/ghostrace-metadata-plan.json
GHOSTRACE_PLAN_DIGEST=$(jq -r '.plan_digest' /tmp/ghostrace-metadata-plan.json)
python3 scripts/roadmap.py github-apply --yes \
  --forge-plan /tmp/ghostrace-forge-plan.json \
  --forge-tasks "$GHOSTRACE_FORGE_TASKS" \
  --plan-digest "$GHOSTRACE_PLAN_DIGEST"
```

The metadata apply re-runs Forge's read-only plan before writing, mutates only the
exact inspected metadata plan, uses field-scoped body, label, assignee, and milestone
operations, preserves unmanaged labels and assignees, and pauses between writes.
Existing managed label or milestone definition drift is a blocker rather than an
automatic overwrite. Creating milestones can require a second plan/apply pass because
issue metadata cannot name a milestone number until GitHub creates it. Regenerate and
inspect the plan until it contains zero operations. CI validates the local graph;
GitHub parity remains a separate release check because it requires authenticated
external state.

## Research basis

The roadmap uses primary platform and standards constraints rather than assuming an
API provides stronger evidence than documented:

- [Apple FSEvents flags](https://developer.apple.com/documentation/coreservices/1455361-fseventstreameventflags)
  expose drop, wrap, root-change, mount, and item-status evidence that must affect
  coverage.
- [Apple TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
  distinguishes macOS Keychain implementations and the user-login limitation of the
  data-protection Keychain.
- [Apple notarization guidance](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
  requires current Developer ID and hardened-runtime practices and the supported
  notary tooling.
- [SQLite WAL](https://www.sqlite.org/wal.html) documents the one-writer model,
  sidecars, checkpoint starvation, same-host requirement, and safe shutdown behavior.
- [SQLite pragmas](https://www.sqlite.org/pragma.html) document integrity checking,
  optimization, and the limits of secure deletion.
- [Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)
  and [extension messaging security](https://developer.chrome.com/docs/extensions/develop/concepts/messaging)
  define the stdio host boundary and require untrusted-message validation.
- [W3C PROV-O](https://www.w3.org/TR/prov-o/) provides an interchange vocabulary;
  GHOSTRACE uses a conservative profile rather than strengthening claims.
- [SLSA 1.2](https://slsa.dev/spec/v1.2/) distinguishes build provenance and its
  assurance levels; releases claim only verified properties.
- [NIST SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final) informs the secure
  development and vulnerability-response program.

These sources constrain tasks and decisions. They are not claims that the current
fixture-only release already implements later milestones.
