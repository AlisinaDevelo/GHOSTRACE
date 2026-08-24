# ADR 0004: Enforce the fixture path without a network

## Status

Accepted for M0 fixture-only development.

## Decision

The fixture, explanation, and export path must run inside an explicit network
denial boundary. The hosted lane uses Docker's `--network=none` with the
`rust:1.88.0-slim-bookworm` Linux amd64 image pinned to
`sha256:a6cab604fa016ac022e78c24038497eb7617ab59150ca4c3dd2ede0fbd514d4b`.
The workflow action references are pinned to commit SHAs. Locked dependencies are
fetched before the boundary and mounted read-only; the denied process sets
`CARGO_NET_OFFLINE=true` so a missing cache is a failure rather than a network
fallback.

The checked-in canary attempts a TCP connection to loopback port 9 under the macOS
sandbox and to TEST-NET address `198.51.100.1:80` under a Linux namespace. It accepts
only the error classes produced by an isolated runner (`PermissionDenied` for the
macOS sandbox and `NetworkUnreachable`, `HostUnreachable`, or `PermissionDenied`
for a Linux network namespace). A normal `ConnectionRefused`, timeout, or successful
connection fails the canary. This prevents a reachable loopback interface, a closed
port, or a silently skipped test from being reported as network denial.

## Local target constraint and equivalent test

The target device has the Docker CLI but no running Docker Desktop daemon, so the
Docker mechanism cannot be claimed as local evidence this week. The reproducible
macOS equivalent is the system sandbox profile
`(version 1) (allow default) (deny network*)`, installed by
`scripts/offline-network-test.sh`. It runs the same ignored canary, the privacy
regression corpus, and the complete locked test suite with no network access. The
canary must observe `PermissionDenied`; the local lane fails if the profile is
missing or if the probe behaves like an ordinary closed-port connection.

This is an enforcement limitation, not a product-path exemption: the hosted Docker
lane remains required before release evidence can claim Linux runner coverage.

## Consequences

- Dependency resolution and runtime execution are separate, auditable phases.
- The test-only canary is deliberately ignored by the ordinary suite because it
  requires a runner-installed denial boundary; the offline lane invokes it with
  `--ignored` and required environment markers.
- A hosted runner image or action revision change requires an explicit diff to this
  ADR and the workflow.
