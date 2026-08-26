# GHOSTRACE

[![CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/workflows/ci.yml/badge.svg)](https://github.com/AlisinaDevelo/GHOSTRACE/actions/workflows/ci.yml)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![MSRV: 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-informational.svg)](rust-toolchain.toml)
[![Status: controlled collector API](https://img.shields.io/badge/status-controlled--collector--API-orange.svg)](docs/ROADMAP.md)

GHOSTRACE is a local macOS event provenance journal. It records bounded,
user-authorized evidence about changes—not everything a person does—and explains
which observations support each sequence.

> **Status:** incubation / M2 selected-root collector API headstart (0.0.1). The API
> requires an explicit consent confirmation and writes only bounded filesystem metadata through the
> existing writer; the ambient `capture` command remains intentionally disabled until
> path-race containment, cursor recovery, and release gates are complete. This
> repository makes no legal chain-of-custody claim.

## Product boundary

GHOSTRACE answers one question: **which bounded observations support the sequence of
changes observed over time?** Its primary object is an event observation with source,
provenance, evidence quality, and coverage limits. It is a journal and explanation
layer; it is not a general-purpose search index or source-code analyzer.

Within the surrounding project portfolio, the boundaries are deliberate:

- **LOOM** retrieves exact passages and visual evidence from user-selected files. It
  does not replace GHOSTRACE's event journal or evidence-linked change explanation.
- **STRATA** and **CARTOGRAPH** analyze TypeScript source and Git revisions to report
  architecture changes. They do not collect macOS events or establish runtime
  causality.

These tools may eventually exchange explicit, user-requested artifacts, but GHOSTRACE
does not silently index their inputs, execute their analyzers, or depend on their
databases. See [Product boundaries](docs/BOUNDARIES.md) for the comparison and the
current limits.

## Ten-minute demo

The current vertical slice is offline and fixture-driven. It does not ask for macOS
permissions, start a collector, contact a service, or upload data.

~~~sh
git clone https://github.com/AlisinaDevelo/GHOSTRACE.git
cd GHOSTRACE
rustup toolchain install 1.88.0
cargo +1.88.0 build

# Print the versioned event contract.
cargo +1.88.0 run -- schema

# Select the terminal event so the explanation includes the full chain and its gap.
EVENT_ID="00000000-0000-4000-8000-000000000008"

# Initialize a private, durable fixture journal. The parent directory must be
# private (mktemp -d creates one with mode 0700).
JOURNAL_DIR="$(mktemp -d)"
JOURNAL="$JOURNAL_DIR/journal.sqlite3"
cargo +1.88.0 run -- init --journal "$JOURNAL"

# Ingest the checked-in synthetic fixture into SQLite, then explain it after
# reopening the journal in a separate process.
cargo +1.88.0 run -- ingest \
  --journal "$JOURNAL" \
  --fixture fixtures/causal-chain.jsonl
cargo +1.88.0 run -- explain \
  --journal "$JOURNAL" \
  --event "$EVENT_ID"

# Inspect a deterministic retention scope before any destructive command. The
# default API policy is 90 days anchored at the command's UTC clock; supplying
# --before makes the receipt reproducible.
cargo +1.88.0 run -- retention-plan \
  --journal "$JOURNAL" \
  --before 2026-01-01T00:00:08Z

# Apply only the exact confirmed scope printed by retention-plan. This is a
# logical deletion; it does not compact SQLite or remove external copies.
# Set the three confirmation variables from the matching JSON receipt above.
cargo +1.88.0 run -- retention-delete \
  --journal "$JOURNAL" \
  --before 2026-01-01T00:00:08Z \
  --confirm-plan "$PLAN_DIGEST" \
  --confirm-candidate-set "$CANDIDATE_SET_DIGEST" \
  --confirm-snapshot-boundary "$SNAPSHOT_BOUNDARY"

# Run bounded SQLite integrity and foreign-key checks. A failed check is a
# recovery stop signal, not an automatic repair request.
cargo +1.88.0 run -- integrity-check \
  --journal "$JOURNAL"

# Verify keyed local state. This detects event edits/insertion/deletion/reorder,
# cursor rollback, policy substitution, and diagnostic tampering without making
# an origin-authenticity claim beyond the local key.
cargo +1.88.0 run -- authenticated-check \
  --journal "$JOURNAL"

# Create a signed, path-free local verification checkpoint. It binds the
# checkpointed database bytes, schema, policy digest, authenticated chain
# position, key generation, integrity receipt, and verification time.
cargo +1.88.0 run -- checkpoint \
  --journal "$JOURNAL"

# The repair command requires a verified source and operates only on a new
# database copy. Intervals are inclusive ingest sequences and must not orphan
# children or cursor tails. The synthetic MVP exercises a safe unreferenced
# interval and prints a before/after manifest without paths or key material.
cargo +1.88.0 run -- recovery-demo

# Inventory residue classes without printing journal or backup paths. This is
# explanatory and read-only; it does not delete or compact anything.
cargo +1.88.0 run -- residue-report \
  --journal "$JOURNAL"

# Export a user-requested, local JSONL view. Existing files are protected.
cargo +1.88.0 run -- export \
  --journal "$JOURNAL" \
  --output /tmp/ghostrace-export.jsonl

# The baseline refuses ambient capture by design.
if cargo +1.88.0 run -- capture; then
  echo "capture unexpectedly succeeded" >&2
  exit 1
else
  echo "capture refused as expected"
fi
~~~

The demo output labels evidence as direct, contextual, inferred, or unknown, and
surfaces gaps instead of filling them with a guess. The same fixture and event ID
produce the same explanation after a process restart. `demo --fixture ...` remains
available as an in-memory shortcut. The durable CLI path uses a deterministic
synthetic key only for this fixture-only headstart; it is not a production
encryption or key-management claim.

## What is shipped now

| Surface | M0 status |
| --- | --- |
| Fixture JSONL ingestion and validation | Available for the developer headstart |
| ghostrace init --journal <path> | Available; creates an idempotent durable fixture journal |
| ghostrace ingest --journal ... --fixture ... | Available; persists a checked-in fixture batch |
| ghostrace explain --journal ... --event <uuid> | Available; deterministic after reopen |
| ghostrace retention-plan --journal ... [--before ...] [--source ...] [--root-id ...] [--retain-at-most-events ...] [--retain-at-most-bytes ...] | Available; read-only dry-run with an authenticated snapshot boundary, candidate digest, coverage gaps, key generations, and conservative encrypted-payload byte estimate |
| ghostrace retention-delete --journal ... [selectors] --confirm-plan ... --confirm-candidate-set ... --confirm-snapshot-boundary ... | Available; transactional logical deletion bound to one retention plan; refuses scope changes and leaves compaction/external copies separate |
| ghostrace residue-report --journal ... [--backup <path>] | Available; path-free residue inventory and explicit logical/compaction/cryptographic/external-copy guarantees; read-only |
| ghostrace integrity-check --journal ... | Available; bounded SQLite integrity/foreign-key checks with path-free recovery guidance; read-only |
| ghostrace authenticated-check --journal ... | Available; keyed canonical-state verification for events, replay detection, cursors, policy history, diagnostics, and explicit deletion boundaries; local-key-only validity |
| ghostrace checkpoint --journal ... | Available; signs a path-free checkpoint binding database identity, integrity receipt, schema, policy set, key generation, chain position, and verification time |
| ghostrace repair --journal ... --destination ... --interval source:start:end | Available; copies a verified source, applies bounded unreferenced interval repair, appends one repair gap per interval, and prints a reconciled before/after manifest |
| ghostrace recovery-demo | Available; device-side MVP of signed checkpoint plus verified-copy repair with privacy and count assertions |
| ghostrace demo --fixture ... --event <uuid> | Available |
| ghostrace preview --journal ... --output ... [--force] | Available; prints the bounded query, field inventory, policy, snapshot, coverage, and destination-class receipt before declassification |
| ghostrace export --journal ... --output ... --confirm-plan ... --confirm-snapshot ... [--force] | Available; requires the matching preview digests, then decrypts and writes one bounded record at a time before atomically publishing a validated 0600 artifact |
| ghostrace preview/export --fixture ... | Available in-memory shortcut with the same explicit confirmation gate |
| ghostrace validate --export ... | Available; rejects incomplete, mixed-version, or digest-drifted JSONL before consumption |
| ghostrace schema | Available |
| ghostrace parquet-profile | Available; prints and validates the strict v1 profile for a future derived Parquet archive; no archive is created |
| ghostrace shell-schema | Available; prints the strict v1 metadata-only contract for a future explicit shell wrapper; no shell is executed |
| ghostrace capture | Refuses by design |
| Local journal and bounded durable writer | Scaffolded for the fixture path; live ingestion is gated |
| Selected-root FSEvents collector API | Available only behind explicit consent; no ambient CLI |
| Storm/lifecycle corpus and native-safe macOS receipt | Available as a bounded test contract; sleep/wake, logout, and volume detach are explicit no-go rows |
| Reproducible filesystem benchmark corpus | Available as an offline synthetic workload contract; native results require the named macOS device and retain observed gaps/failures |
| Event-storm backpressure and loss accounting | Available as a bounded synthetic stress contract; queue pressure exposes pending limits, an emergency status slot, auditable gaps, and recovery-required state |
| Snapshot-consistent query pagination | Available as a bounded library API; encrypted page tokens bind policy scope, source/root/kind/time filters, the versioned ordering contract, schema, and an ingest snapshot boundary; policy-blocked summaries stay in coverage metadata rather than query events, and every page carries gap-aware coverage unless explicitly opted out |
| Evidence-claim grammar | Available as a versioned bounded renderer; templates preserve evidence labels and cited event IDs, expose gap limits, and refuse intent, completeness, process-attribution, causality, and unsupported rename claims |
| Cross-source correlation rule registry | Available as a versioned, policy-bounded adjacency rule; unknown coverage, unsupported scope, and clock skew abstain instead of becoming positive evidence |
| Explanation determinism and counterexamples | Available as an offline golden/property/mutation matrix; every claim template and evidence level is exercised, ordering/page permutations are compared, and required-observation removal must downgrade or remove a claim |
| Export schema and manifest registry | Available as six strict v1 contracts with stable IDs, golden examples, version declarations, fail-closed streaming validation for mixed versions, counts, bytes, and body digests, and bounded record/metadata limits |
| Derived Parquet archive profile | Available as a strict, lossless 23-column v1 contract with explicit gap/provenance/policy mappings, additive-nullable evolution gates, bounded rows/metadata, and privacy-safe storage defaults; a writer remains a later task |
| Explicit shell metadata schema | Available as a strict v1 contract for wrapper session, executable identity, sanitized working-directory identity, timing, outcome, exit code, and signal; raw command state is structurally rejected |
| Shell wrapper lifecycle contract | Available as synthetic reference tests for child status propagation and explicit incomplete-execution gaps; no shell executor is shipped |
| Shell secret-leakage red-team contract | Available as synthetic negative tests for metadata, journal, diagnostics, exports, panic output, and documented OS exposure; no shell capture is shipped |
| Git repository/worktree identity contract | Available as a path-free metadata contract with object-database/worktree digests, selected-root/source-scope binding, and move/clone/linked-worktree/submodule/bare/reinitialization continuity tests; no Git command runner or remote access is shipped |
| Metadata-only Git snapshot contract | Available as a strict algorithm-aware SHA-1/SHA-256 snapshot boundary with bounded status/operation facts and explicit partial-history, replace-ref, shallow, submodule, and alternate-object-database limitations; no object content is read |
| Shell, Git, frontmost-app, or browser collectors | Not shipped |
| macOS Keychain-backed production encryption | Not shipped |
| Signed/notarized release artifacts | Not shipped |

The roadmap is a plan, not a promise. A capability is shipped only when its privacy,
failure, and coverage tests are present.

### Temporal ordering contract

The display order contract is version `1`: known source observation time, durable
`ingest_seq`, then canonical `event_id`. The same key is used by query pages and
JSONL export. Source observation time, local receipt time, and optional
process-local monotonic sequence are retained as distinct timing evidence; none
is a causal proof. Equal timestamps, source clock rollback, delayed delivery,
sleep-sized ingest gaps, and missing source time are surfaced as temporal
ambiguity, with ingest sequence used only as the explicit fallback.

### Evidence-claim grammar

Claim grammar version `1` maps each event kind to a typed template with required
facts, an evidence-label rule, prohibited implications, and gap behavior. Claims
are rendered in bounded `en` or `en-GB` locales and cite the event ID in both the
structured citation list and the text. Rename events explicitly leave old-to-new
identity unknown; no template asserts intent, completeness, process attribution,
or causality.

### Cross-source correlation rules

Correlation rule registry version `1` exposes the bounded
`cross_source_temporal_adjacency` rule. Its descriptor lists the only fields it
may inspect (opaque event ID, source, kind, observed time, evidence level, policy
scope, and coverage markers), a 60-second window and 256-event input bound, its
exclusions, inferred/unknown output policy, and five counterexample fixture
classes. Evaluation authorizes each event against the query policy before the
rule sees it. A denied scope, unknown evidence or coverage interval, same-source
pair, equal timestamp, or clock rollback yields an explicit unknown result rather
than a positive relationship. Rule and registry versions are included in
explanation identities and export manifests so historical output remains
reproducible.

## Architecture

The evidence path is deliberately small:

~~~text
fixture JSONL (now) ─────────┐
selected-root FSEvents (now) ├─> normalization ─> deny-by-default policy
                             ┘                         │
                                               v
                                  versioned event + provenance
                                               │
                                               v
                                  bounded SQLite WAL writer
                                               │
                                               v
                           deterministic explain / explicit export
~~~

Each accepted event carries source facts, timestamps, policy context, and evidence
quality. A gap is a first-class record. Explanations cite the observations they use
and state when the source cannot establish completeness. See
[Architecture](docs/ARCHITECTURE.md) and [Event model](docs/EVENT_MODEL.md).

Journal ingestion also requires a typed adapter-origin capability. Fixture, live,
import, and repair paths own separate provenance namespaces and allowed event classes;
deserializing a fixture cannot grant a live-collector capability.

The Git identity headstart is similarly bounded. `GitIdentity` retains only
object-database/worktree digests, an opaque selected-root ID, source scope, and
repository kind. It classifies moves, clones, linked worktrees, submodules, bare
repositories, and reinitialization without retaining remote URLs, credentials,
configuration, reflog messages, or paths. It is a contract for a future explicit Git
adapter; no Git command runner, remote access, or authorship/causality claim is shipped.

## Trust contract

GHOSTRACE is designed around a narrow local boundary:

- **Local-only:** source inspection of current product and runtime paths finds no
  network client, telemetry, cloud sync, URL fetching, or silent upload path. Task
  0044 must make that boundary independently enforceable in CI before it becomes
  release evidence. The separate maintainer-only roadmap synchronizer invokes `gh`
  only when an operator explicitly runs its GitHub commands.
- **User-authorized:** the selected-root collector requires explicit consent, selected
  scope, and a versioned policy. No event is retained before policy evaluation.
- **Minimized:** the baseline records bounded metadata about changes. It does not
  read file contents as part of the filesystem design.
- **Honest evidence:** direct, contextual, inferred, and unknown evidence levels are
  distinct. Missing coverage is visible.
- **Fail closed:** the ambient `ghostrace capture` command refuses while the gates for
  path-race containment, cursor recovery, and release protection are incomplete; the
  library collector itself cannot start without a consumed consent confirmation.
- **Inspectable:** exports are explicit commands. Existing destinations are not
  overwritten unless --force is supplied. Exporting streams through private
  bounded temporaries, fsyncs before rename, validates the complete manifest and
  body before publication, and removes or abandons only an unmistakably
  incomplete temporary on cancellation or write failure.

The initial product does **not** use keylogging, microphones, screen recording,
clipboard capture, window titles, page contents, or private-browsing data by default.
It does not require root, Full Disk Access, Accessibility, or Automation permissions.
No silent upload mechanism exists in the current source; the planned network-denial
CI lane will continuously verify that boundary.

FSEvents is a change-notification source, not a complete process-attributed causal
trace. It can omit, coalesce, reorder, or delay observations. Endpoint Security is
optional and deferred; it is entitlement-gated and will not silently become part of
the baseline.

## Non-goals

GHOSTRACE is not an employee-monitoring product, keylogger, screen recorder, content
indexer, browser-history default, malware detector, remote agent, cloud analytics
service, or legal evidence-preservation system. It does not infer intent from absent
events and does not claim that a recorded sequence proves causality.

## Project layout

~~~text
src/                 Rust library and CLI
fixtures/            Synthetic, non-user event fixtures
docs/                Architecture, privacy, threat, platform, and research notes
docs/adr/            Immutable architecture decisions
.github/             CI, dependency checks, issue forms, and contribution templates
~~~

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — data path, boundaries, and failure behavior
- [Privacy](docs/PRIVACY.md) — data inventory, defaults, consent, and export rules
- [Threat model](docs/THREAT_MODEL.md) — assets, STRIDE analysis, and residual risk
- [Event model](docs/EVENT_MODEL.md) — evidence levels, provenance, and gaps
- [Product boundaries](docs/BOUNDARIES.md) — the event-journal boundary and portfolio comparison
- [Evaluation](docs/EVALUATION.md) — correctness, privacy, and performance gates
- [FSEvents lifecycle corpus](fixtures/fsevents-lifecycle-corpus-v1.json) — ground truth, coalescing, gaps, and guarded device rows
- [Temporal ordering fixture](fixtures/temporal-ordering-v1.json) — clock skew, delayed delivery, tie-breaking, and missing source time
- [Query coverage fixture](fixtures/query-gap-coverage-v1.json) — nested, adjacent, open-ended, and cross-source gap intervals
- [Correlation rule fixture](fixtures/correlation-rules-v1.json) — positive, negative, ambiguous, adversarial, and clock-skew cases
- [Explanation counterexample fixture](fixtures/explanation-counterexamples-v1.json) — claim-template/evidence/gap matrix, conflict outcomes, and required-observation mutations
- [Export schema registry](schemas/export-registry-v1.json) — strict manifest, event, gap, claim, policy, and source-coverage contracts
- [Export contract goldens](fixtures/export-manifest-v1.golden.json) — manifest example; sibling `*-record-v1.golden.json` files cover every registered record
- [Parquet archive profile](schemas/parquet-archive-profile-v1.json) — strict derived-archive contract; [golden profile](fixtures/parquet-archive-profile-v1.golden.json)
- [Explicit shell metadata schema](schemas/shell-execution-metadata-v1.json) — wrapper-only metadata contract; [golden record](fixtures/shell-execution-metadata-v1.golden.json)
- [Shell wrapper lifecycle fixture](fixtures/shell-wrapper-lifecycle-v1.json) — synthetic status, timeout, cancellation, and explicit gap contract
- [Shell secret-leakage fixture](fixtures/shell-secret-leakage-v1.json) — synthetic unique-sentinel corpus for denied shell channels and documented external OS exposure
- [Git repository/worktree identity fixture](fixtures/git-repository-worktree-identity-v1.json) — path-free object-database/worktree continuity matrix for move, clone, linked worktree, submodule, bare, scope rebinding, and reinitialization
- [Metadata-only Git snapshot contract](docs/GIT_SNAPSHOT.md) — algorithm-aware object IDs, bounded status/operation facts, explicit source limitations, and excluded Git metadata
- [Git snapshot golden fixture](fixtures/git-snapshot-metadata-v1.golden.json) — deterministic metadata-only snapshot example
- [Filesystem benchmark corpus](fixtures/filesystem-benchmark-corpus-v1.json) — bounded synthetic trees and native measurement contract
- [Reproducibility](docs/REPRODUCIBILITY.md) — pinned toolchain, fixture provenance, and clean-machine smoke
- [Research](docs/RESEARCH.md) — landscape, differentiation, and primary sources
- [Identity gate](docs/IDENTITY.md) — qualified descriptor, release identifiers, and legal-review boundary
- [Platform](docs/PLATFORM.md) — macOS boundary and permission policy
- [Roadmap](docs/ROADMAP.md) — 160 tasks across M0 through M11, August 2026–December 2031
- [ADR 0001](docs/adr/0001-local-only-minimized-capture.md) — local-only minimized capture
- [ADR 0002](docs/adr/0002-fsevents-before-endpoint-security.md) — FSEvents before Endpoint Security
- [ADR 0003](docs/adr/0003-sqlite-wal-active-journal.md) — SQLite WAL active journal

For how to change the project, see [CONTRIBUTING.md](CONTRIBUTING.md). For a suspected
vulnerability, see [SECURITY.md](SECURITY.md). Questions and feature proposals belong
in the issue forms, not in a journal attachment.

## Development checks

Use the pinned toolchain and run the core checks exercised by CI:

~~~sh
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.88.0 test --locked --all-targets --all-features
python3 scripts/roadmap.py check
python3 scripts/reproducibility.py check
python3 scripts/fixture-manifest.py check
python3 scripts/filesystem-benchmark.py check
python3 -m unittest discover -s tests -p 'test_roadmap.py' -v
python3 scripts/roadmap.py index > /tmp/ghostrace-roadmap-index.md
diff -u .forge/tasks/README.md /tmp/ghostrace-roadmap-index.md
scripts/reproducibility-test.sh
~~~

The fixture path should remain offline. Do not add a network dependency, a permission
request, a new sensitive field, or a collector behavior without updating the privacy and
threat documentation and adding device regression coverage.

## License

GHOSTRACE is licensed under the [Mozilla Public License 2.0](LICENSE). Third-party
dependencies retain their own licenses; dependency policy is checked in
[deny.toml](deny.toml).
