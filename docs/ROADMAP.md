# Roadmap

This roadmap spans the first public contract in August 2026 through a possible
v1.0 readiness gate in August 2029. Dates are planning windows, not promises. Every
milestone can stop at a refusal if its privacy, correctness, or recovery evidence is
not ready.

| Milestone | Window | Exit focus |
| --- | --- | --- |
| M0 | Aug–Sep 2026 | Public contract, fixture-only CLI, documentation, CI |
| M1 | Oct 2026–Mar 2027 | Versioned envelope, policy gate, encrypted storage design, bounded writer, replay |
| M2 | Apr–Sep 2027 | Selected-root FSEvents, canonical paths, cursor recovery, backpressure, benchmarks |
| M3 | Oct 2027–Mar 2028 | Stable queries, deterministic explain, JSONL manifest, retention, integrity, optional archive |
| M4 | Apr–Sep 2028 | Explicit shell/Git metadata and frontmost-app integrations |
| M5 | Oct 2028–Mar 2029 | Browser security decision, explicit local transport, optional adapters, read-only timeline |
| M6 | Apr–Aug 2029 | Release signing/notarization, SBOM, performance gates, compatibility and incident readiness |

## M0 — public, fixture-only headstart

- Publish the local-only product contract and permanent non-goals.
- Complete the identity and namespace gate: record collision evidence, choose whether
  to keep or rename GHOSTRACE, and reserve only the namespaces that decision authorizes.
- Keep live capture refused.
- Exercise synthetic causal fixtures through validation, explanation, schema output,
  and explicit JSONL export.
- Run format, Clippy, test, dependency, advisory, and license/source checks.

## M1 — durable evidence core

- Freeze the versioned canonical event envelope and evidence levels.
- Implement deny-by-default consent, selected scope, exclusions, and redaction.
- Add production macOS Keychain-backed authenticated encryption with a deterministic
  test provider.
- Add SQLite WAL migrations, a single bounded writer, atomic cursor commits, and
  crash/replay tests.

## M2 — first live source

- Add opt-in selected-root FSEvents metadata collection.
- Enforce canonical paths, symlink containment, exclusions, and bounded payloads.
- Persist cursors and emit gaps for invalid, wrapped, dropped, or uncovered history.
- Measure event storms, latency, duplicates, omissions, ordering, and recovery.

## M3 — portable analysis

- Add stable time-window queries and evidence-linked deterministic explanations.
- Version JSONL export manifests and make coverage and gaps portable.
- Add explicit retention, deletion, integrity checks, and an optional Parquet archive.
- Keep exports user initiated and warn when plaintext leaves the journal.

## M4 — explicit developer integrations

- Add a shell wrapper that records only bounded execution metadata the user
  deliberately routes through it.
- Add opt-in Git snapshots and hooks without capturing command arguments, secrets,
  standard input, or output.
- Add frontmost-application metadata only after attribution and privacy tests.

## M5 — constrained local interfaces

- Decide browser transport, pairing, allowlists, private-context handling, and
  message limits in a security ADR before shipping any browser adapter.
- Add a versioned Unix-domain local service with least privilege.
- Add read-only timeline and explanation UI with a strict capability allowlist.
- Keep browser page contents, credentials, and private browsing out of the default.

## M6 — release and readiness

- Produce locked, reproducible macOS builds for supported architectures.
- Sign and notarize release artifacts and publish an SBOM.
- Establish measured CPU, memory, ingest, journal, query, and export budgets.
- Run a separate go/no-go evaluation of optional Endpoint Security attribution. Its
  entitlement and permission footprint must remain explicit, high privilege, and
  rejectable without weakening the FSEvents baseline.
- Verify schema upgrades, export compatibility, privacy regressions, rollback, and
  incident recovery before calling the product v1-ready.

The sequence is intentionally conservative: no later milestone turns on live capture
until the gates in the earlier milestones are demonstrated.
