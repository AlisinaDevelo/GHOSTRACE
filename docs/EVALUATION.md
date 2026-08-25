# Evaluation plan

GHOSTRACE is evaluated as a bounded evidence system, not as a universal activity
recorder. A passing test must show both what the system observed and what it
refused to claim.

## Current fixture gates

The M0 developer headstart should prove:

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

## Future live-source gates

No live collector is ready until its fixture, integration, and failure matrix covers:

| Area | Required evidence |
| --- | --- |
| Consent | Deny-by-default, explicit scope, versioned policy, stop/revoke behavior |
| Minimization | Field inventory, redaction tests, prohibited-field regression |
| Scope | Root canonicalization, symlink containment, exclusions, malformed Unicode/path tests |
| Coverage | Source flags, cursor continuity, coalescing/omission behavior, first-class gaps |
| Recovery | Crash before/after commit, cursor/event atomicity, restart discontinuity |
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

## Test layers

1. **Unit tests:** model validation, URL sanitization, policy decisions, evidence
   labels, encryption failure modes, and path rules.
2. **Property tests:** malformed and attacker-shaped inputs, policy state transitions,
   and bounded-size behavior.
3. **Fixture tests:** deterministic replay, explanation citations, gap propagation,
   export compatibility, and privacy regression.
4. **Integration tests:** SQLite migrations, WAL behavior, permissions, crash
   injection, cursor/policy/diagnostic atomicity, FIFO acknowledgements, bounded
   queue policies, cancellation, and retry limits.
5. **Platform tests:** selected-root FSEvents behavior and macOS permission changes.
6. **Release checks:** locked build, advisories, license/source policy, SBOM,
   signing/notarization, and network-surface review.

## Evidence package

Each future milestone should publish the tests and limitations supporting its exit
claim. A benchmark or green CI run is not evidence that a source is complete. The
report must name gaps, excluded contexts, unsupported platforms, and untested
failure paths.
