# Evaluation plan

GHOSTRACE is evaluated as a bounded evidence system, not as a universal activity
recorder. A passing test must show both what the system observed and what it
refused to claim.

## Current fixture gates

The M0 developer headstart should prove:

- the durable fixture CLI can initialize a private journal, ingest a checked-in
  fixture, reopen it for explanation, and export a versioned JSONL stream;
- repeating initialization is idempotent, and explanation bytes remain stable across
  process boundaries;
- valid causal-chain fixtures parse into the versioned event envelope;
- malformed lines, unknown schema versions, invalid UUIDs, and invalid payloads fail
  closed without retaining the rejected value;
- the same fixture and event ID produce byte-stable explanation structure;
- explanations cite event IDs and label direct, contextual, inferred, and unknown
  evidence;
- gap and policy-blocked records appear in coverage summaries and exports;
- export refuses an existing destination unless the user supplies --force;
- browser-shaped fixture URLs lose userinfo, query, and fragment data before storage;
- private-context events are rejected by default;
- no prohibited field appears in serialized events, diagnostics, or exports;
- the capture command returns the documented refusal;
- tests pass in a network-denied environment.
- named storage fault schedules return or terminate at every journal durable
  boundary, then reopen and assert committed rows, cursor state, stable fixture
  key generation, visible gaps, and retry/idempotence;
- bounded seed replay and minimized schedules remain checked-in regression inputs.

## Future live-source gates

The selected-root FSEvents API is a bounded first slice, not a release-ready ambient
collector. No ambient collector is ready until its fixture, integration, and failure
matrix covers:

| Area | Required evidence |
| --- | --- |
| Consent | Deny-by-default, explicit scope, versioned policy, stop/revoke behavior |
| Minimization | Field inventory, redaction tests, prohibited-field regression |
| Scope | Root canonicalization, descriptor-backed no-follow symlink/hard-link containment, exclusions, malformed Unicode/path tests |
| Coverage | Source flags, cursor continuity, coalescing/omission behavior, first-class gaps |
| Recovery | Crash before/after commit, cursor/event atomicity, restart discontinuity, idempotent replay, explicit reset/wrap/invalidation |
| Backpressure | Bounded memory under bursts, measurable drops, visible loss records |
| Security | Key-handling tests, restrictive file permissions, log-redaction tests, dependency and advisory checks |
| Platform | Supported macOS versions and architectures, permission prompts, revocation |
| Export | Stable schema, coverage manifest, plaintext warning, overwrite protection |

## Measurements

Benchmark reports must name the hardware, macOS version, Rust version, fixture or
workload, journal size, collector mix, warm-up method, and build profile. Measure:

- source-to-journal latency distribution;
- duplicate, reordered, coalesced, delayed, missing, and attributed events;
- restart recovery and cursor replay;
- bounded queue depth, admission wait, retry attempts, cancellation, and explicit
  loss/gap accounting;
- idle CPU, resident memory, WAL growth, query time, and export throughput.

Performance budgets are not declared until a representative workload exists. A
number without its workload would be a misleading guarantee.

## Storm and lifecycle corpus (task 0075)

[`fixtures/fsevents-lifecycle-corpus-v1.json`](../fixtures/fsevents-lifecycle-corpus-v1.json)
is the versioned ground-truth contract for high-rate and lifecycle evaluation.
It names the operation log, direct observations, permitted FSEvents coalescing,
required coverage gaps, recovery expectation, and per-scenario resource bound for
bulk checkout, package-like atomic install, rename storm, recursive directory
deletion, sleep/wake, logout, volume detach, process termination, and collector
restart. `python3 scripts/fsevents-lifecycle-corpus.py check` replays the corpus 32
times with a pinned seed and emits omission, duplicate, ordering, recovery, and
resource distributions. The replay report is synthetic validation of the reporting
contract; it is not a native-device observation.

The macOS integration receipt runs the six non-disruptive rows (bulk checkout,
package install, rename storm, directory deletion, process kill, and restart) on a
private temporary selected root. It records operation counts, source-ID duplicate
handling, ordering inversions, restart recovery, callback/drop counters, and
path-free output. Sleep/wake, logout, and volume detach remain explicit guarded
no-go rows because triggering them would suspend or terminate the user's active
session or risk user data. They are never converted into a pass by hosted CI or a
synthetic replay; a future authorized interactive run must attach separate native
evidence and verify the required gap before those rows can close.

## Reproducible filesystem benchmark corpus (task 0076)

[`fixtures/filesystem-benchmark-corpus-v1.json`](../fixtures/filesystem-benchmark-corpus-v1.json)
defines eight synthetic, offline workload families: small, deep, wide, Unicode,
case-variant, Git, build-output, and event-storm trees. The generator is bounded
by entry, file-byte, run-time, and journal-growth limits; it never reads or retains
user file contents or paths. Validate the contract with
`python3 scripts/filesystem-benchmark.py check`.

The native device command is
`python3 scripts/filesystem-benchmark.py run --profile release`. It runs each
workload three times on the selected private root and reports latency percentiles,
direct/contextual/inferred/unknown coverage classes, duplicate and gap rates, CPU,
resident memory, journal disk growth, and the available power-telemetry delta.
An observed cursor regression is retained as `cursor_regression` rather than
converted into a pass; FSEvents remains a change-notification source and does not
prove process causality. Energy is a value only when the unprivileged
`IOPMPowerSource` counter advances; privileged `powermetrics` is never substituted.
Hardware and OS are part of the receipt, and results are not cross-machine
comparable without an explicit normalization study.

## Test layers

1. **Unit tests:** model validation, URL sanitization, policy decisions, evidence
   labels, encryption failure modes, and path rules.
2. **Property tests:** malformed and attacker-shaped inputs, policy state transitions,
   ordered cursor reordering/replay, skipped and opaque cursors, and bounded-size behavior.
3. **Fixture tests:** deterministic replay, explanation citations, gap propagation,
   export compatibility, and privacy regression.
4. **Integration tests:** SQLite migrations, WAL behavior, permissions, named
   fault injection and reopen replay, cursor/policy/diagnostic atomicity, FIFO
   acknowledgements, bounded queue policies, cancellation, and retry limits.
5. **Platform tests:** selected-root FSEvents behavior and macOS permission changes.
6. **Release checks:** locked build, advisories, license/source policy, SBOM,
   signing/notarization, and network-surface review.

Task 0063 adds a platform lifecycle gate beneath the selected-root collector:
macOS integration tests exercise creation, owner-run-loop scheduling, callback
copying, start/stop/restart, flush, invalidation, and explicit refusal states.
Task 0013 adds the first live-source gate: explicit consent confirmation, exact
root mapping, path-free filesystem payloads, lifecycle records, writer admission,
controlled create/modify/move/delete integration, and revocation before pending
events commit. Task 0068 adds the descriptor-backed no-follow walk for later opens;
cursor/recovery, exclusion, and ambient capture work below remains separate.
Cross-platform lifecycle-model tests inject schedule/start failures and assert that
stop, invalidate, and release occur exactly once. AddressSanitizer is a required
macOS evidence lane when the pinned nightly sanitizer toolchain is available; a
missing toolchain or unavailable architecture is recorded as a no-go, never as a
passing substitute.

## Evidence package

Each future milestone should publish the tests and limitations supporting its exit
claim. A benchmark or green CI run is not evidence that a source is complete. The
report must name gaps, excluded contexts, unsupported platforms, and untested
failure paths.

Task 0062 retains the bounded schedule fixture and the device receipt in
`docs/evidence/0062-storage-fault-matrix.md`. It verifies the current fixture
journal's single deterministic key generation. Task 0055 separately verifies the
key-free envelope, resumable rotation, and explicit destruction receipts; it does not
claim a live collector or signed-Keychain availability.

Task 0063 retains its stream lifecycle receipt in
`docs/evidence/0063-fsevents-stream-lifecycle.md`. That receipt must link the
device-local native callback run, the mock partial-initialization counts, the
sanitizer result (or explicit no-go), and the protected-main rerun before the
issue may close.
