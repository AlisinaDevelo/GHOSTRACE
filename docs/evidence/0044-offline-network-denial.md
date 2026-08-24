# Offline network-denial lane evidence

Status: complete for the M0 fixture-only path.

## Scope and artifacts

- `GHOSTRACE-0044-OFFLINE-CODE-477F56F`: pre-merge implementation commit
  `477f56f6074596ff49608a69eddfeeea788289cb`.
- `GHOSTRACE-0044-MERGED-85F653A`: protected-main merge commit
  `85f653aca45b48776853eef83d3a3e183063d54e` (PR #170).
- `GHOSTRACE-0044-LOCAL-PREMERGE-3B13EBF`: enforced macOS lane at the
  pre-merge implementation; raw log SHA-256
  `3b13ebf3631e66cf2bcfe20bf1dd654965542ff58c965a355b2cc89f0b1c26a7`.
- `GHOSTRACE-0044-LOCAL-MERGED-D0BD584`: the same lane rerun from merged main;
  raw log SHA-256
  `d0bd584fb50be928d174a1222071222e7a0ec0b6d565783717d08fea1ca76d2b`.
- `GHOSTRACE-0044-LOCAL-DENY-261704F`: unsandboxed canary negative control;
  it exited 101 with `ConnectionRefused` instead of accepting a false pass.
- `GHOSTRACE-0044-LOCAL-MARKER-7C74BC7`: `--inside` without the enforcement
  marker exited 1 before running tests.
- `GHOSTRACE-0044-DOCKER-DAEMON-591C8BE`: target Docker CLI diagnostic; the
  CLI is installed, but no Docker Desktop daemon is running (exit 1).

The retained implementation is [the offline runner](../../scripts/offline-network-test.sh),
[the canary](../../tests/offline_network_canary.rs), [the workflow](../../.github/workflows/offline-network.yml),
and [ADR 0004](../adr/0004-offline-network-denial.md).

## Acceptance mapping

1. `.github/workflows/offline-network.yml` fetches the locked dependency set
   before the denial boundary, then runs the product path in a digest-pinned
   Rust 1.88.0 Linux amd64 image
   `rust@sha256:a6cab604fa016ac022e78c24038497eb7617ab59150ca4c3dd2ede0fbd514d4b`
   with Docker `--network=none`. The action
   revisions are pinned by commit SHA. The denied process mounts Cargo sources
   read-only and sets `CARGO_NET_OFFLINE=true`; a missing cache cannot silently
   trigger a download.
2. `tests/offline_network_canary.rs` attempts a real TCP connection. macOS
   `sandbox-exec` must return `PermissionDenied`; the Docker/Linux namespace
   probes TEST-NET `198.51.100.1:80` and must return an unreachable/permission
   error. `ConnectionRefused`, timeout, or success fails the test. The canary,
   privacy corpus (fixture, explanation, and export), and complete product suite
   all passed in both enforced local runs.
3. The target has Docker CLI 29.6.2 but no running daemon, so Docker execution
   is not claimed as local evidence. ADR 0004 records that limitation and the
   reproducible macOS equivalent `(version 1) (allow default) (deny network*)`,
   which is what produced the retained local evidence. The hosted lane remains
   the required Linux coverage path; this report does not use Actions as test
   evidence.

## Target and commands

Both runs were performed on a MacBookPro17,1 with Apple M1, 8 GB RAM, macOS
26.6.2 (25G83), Darwin 25.6.0, arm64, `rustc 1.88.0` and `cargo 1.88.0`.

The primary command was:

```sh
scripts/offline-network-test.sh
```

It installed the system sandbox denial, ran the ignored canary, ran the seven-case
privacy regression test (including fixture ingestion, explanation refusal, and
export refusal), and ran all Rust targets/features offline. The complete suite
reported 18 vertical-slice tests plus the privacy test; the canary is intentionally
ignored in the ordinary suite and passed when explicitly invoked by the lane.

Additional checks passed locally: `shellcheck scripts/offline-network-test.sh`,
`actionlint .github/workflows/offline-network.yml`, `cargo +1.88.0 fmt --all --
--check`, `cargo +1.88.0 clippy --locked --offline --all-targets --all-features
-- -D warnings`, roadmap graph validation, roadmap unit tests (23), and generated
index parity. A source scan found no network client or socket API under `src/`;
the only socket code is the explicitly test-only canary.

## Failure and recovery record

The first hosted attempt exposed two defects before merge: the Rust image's
`rustup` proxy attempted channel metadata access after networking was denied, and
Docker's isolated loopback returned `ConnectionRefused` for the initial canary.
The runner now prepends the image's installed 1.88.0 toolchain binaries, and the
Linux canary uses TEST-NET rather than loopback. The corrected workflow passed its
full canary and product lane; the local evidence above remains the acceptance
record because hosted results are intentionally excluded this week.

## Limitations

This proves the fixture-only product path and its test runner boundary. It does not
exercise live macOS capture, permissions, entitlements, Endpoint Security, a running
Docker Desktop daemon on this target, or production deployment networking. No live
journal, export, private content, account name, or secret was used.
