# macOS support and permission matrix

The machine-readable contract is [`tests/fixtures/support-matrix-v1.json`](../tests/fixtures/support-matrix-v1.json).
It is part of the release evidence surface: a passing test on one runner never
silently expands the supported operating-system or architecture set.

## Current device result

`GHOSTRACE-0045-DEVICE-20260824` verifies the fixture-only product on a MacBook Pro
17,1 with an Apple M1 (arm64), macOS 26.6.2 (25G83), and Rust/Cargo 1.88.0. It does
not verify the macOS 15 floor or an Intel Mac. Those rows remain explicitly
`target-unverified` or `no-go-unavailable-hardware` until the required device run
is retained.

## Reading the matrix

- `target` describes an intended compatibility promise, not a test result.
- `verified` is reserved for a retained target-device run with the exact commit,
  OS build, architecture, commands, logs, digests, and limitations.
- `no-go-unavailable-hardware` is a deliberate stop, never a CI substitution.
- Every collector declares required, optional, and prohibited permissions. A
  refusal must be observable, bounded, redacted, and must not create a broader
  permission prompt or retain data from an unauthorized scope.

The annual validation owner, beta/release-candidate cadence, evidence format, and
row-retirement rule live in the JSON contract and are checked by the Rust test
suite.
