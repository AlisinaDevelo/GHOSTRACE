# GHOSTRACE 0045 support-matrix evidence

Artifact ID: `GHOSTRACE-0045-DEVICE-20260824`.

This record is the target-device evidence for the supported macOS and permission
matrix. The matrix deliberately separates a declared target from a verified
device result and records unavailable hardware as a no-go.

## Implementation under test

- Contract: [`tests/fixtures/support-matrix-v1.json`](../../tests/fixtures/support-matrix-v1.json)
- Human-readable policy: [`docs/SUPPORT_MATRIX.md`](../SUPPORT_MATRIX.md)
- Task: `.forge/tasks/0045-publish-the-supported-macos-and-permission-test-matrix.md`
- Issue: [#49](https://github.com/AlisinaDevelo/GHOSTRACE/issues/49)
- Source commit: `acf3fb4ea2d16ad33343bc9162a713c21435f363`
- Contract SHA-256: `4e7583fb843a1faac177ea14d6a7f6e4e61363178228fbf6a1c60858bbd0c462`

## Device and limitation

- Device: MacBook Pro 17,1, Apple M1, 8 GB
- OS: macOS 26.6.2 (25G83), arm64
- Rust/Cargo: 1.88.0
- Verified row: macOS 26 arm64
- No-go rows: Intel (`x86_64`) hardware was not available
- Still unverified: the macOS 15 Sequoia floor on both target architectures

No Actions result is used as evidence. The release gate must retain a separate
beta/release-candidate run for the floor and every target architecture.

## Device pipe

The complete local pipe ran at the source commit above on this device. The
retained log is `/tmp/ghostrace-0045-full.VoSZPU/pipeline.log` with SHA-256
`1f4becb8750d1aa977f89881849446c150c2ff98346f499d4260dc2061194c29`.

- `cargo +1.88.0 metadata --locked --offline` — pass
- Rustfmt, debug and release builds, all-target/all-feature debug and release
  suites — pass (19 integration/fixture tests per profile; 5 support-matrix
  tests; 18 vertical-slice tests; 1 privacy-corpus test)
- Focused support-matrix, privacy-corpus, and doctests — pass
- Clippy with `-D warnings`, `actionlint`, and `shellcheck` — pass
- `scripts/offline-network-test.sh` under macOS `sandbox-exec` — pass: the
  canary observed `PermissionDenied`, privacy corpus passed, and the complete
  locked suite passed with `CARGO_NET_OFFLINE=true`
- Roadmap validation, 23 Python roadmap tests, and generated-index parity — pass
  (160 tasks, 12 milestones, 488 dependency edges, 108 parent edges)
- CLI help/schema/demo — pass; repeated demo output was byte-identical
- Export happy path, overwrite refusal, `--force` recovery, and explicit
  capture refusal — pass
- `cargo-audit` and `cargo-deny` — unavailable on this device; neither was
  replaced with an Actions claim

## Acceptance map

| Acceptance criterion | Retained evidence |
| --- | --- |
| macOS major versions and Intel/Apple-silicon expectations are explicit | `support-matrix-v1.json` `support_floor`, `architectures`, and `platform_matrix` |
| Every collector declares required, optional, prohibited permissions and refusal behavior | `support-matrix-v1.json` `collectors[*].permissions`; `cargo test --test support_matrix` |
| Annual beta/RC validation has owner, evidence format, and retirement rule | `support-matrix-v1.json` `annual_validation`; `cargo test --test support_matrix` |

## Required rerun record

The original implementation was reviewed and merged, and the exact matrix test
and full device pipe were rerun against the then-current merged `main` SHA below.
The continuation rerun after later GHOSTRACE merges is retained in the next
section so this task remains anchored to the current protected-main state.

## Post-merge device rerun

The exact pipe was rerun against the merged product commit
`039db0e47ea943d892da78d218682c670251a55c` on the same target device. The retained
log is `/tmp/ghostrace-0045-merged-full.cKsI9X/pipeline.log` with SHA-256
`bcfb386927da3304c64dbaaa6d3e8422dc464c77aa0de4a3fd0a4a050c6104cc`.

- Device, OS, architecture, and Rust/Cargo versions are unchanged from the source run.
- Debug and release all-target/all-feature suites, focused support-matrix and privacy
  tests, doctests, clippy (`-D warnings`), actionlint, shellcheck, the sandboxed
  offline-network canary, roadmap/index parity, and the CLI demo/export/refusal checks
  all passed.
- The repeated CLI demo remained byte-identical and the unsafe capture path remained
  refused by policy.
- `cargo-audit` and `cargo-deny` remain unavailable locally; no hosted check is used as
  a substitute.
- The temporary verification worktree was removed after the receipt was captured.

Decision: the support-matrix implementation is reproducible and green on the merged
main commit for the verified macOS 26 arm64 row. The macOS 15 floor and Intel row remain
explicit release-gate limitations, not claims of device coverage.

## Current-main continuation rerun

The protected main branch subsequently gained the consent, policy-decision, and
data-protection Keychain slices. To keep this task's evidence current, the complete
device pipe was rerun against `cdb04cdaa4156360b60122d23bf23566bda60d9d` on the same
target device.

- Device: MacBook Pro 17,1, Apple M1, 8 GB, arm64
- OS: macOS 26.6.2 (25G83)
- Toolchain: Rust/Cargo 1.88.0
- Sanitized log: `/private/tmp/ghostrace-0045-current-main-final-20260824.log`
- Log SHA-256: `73b41b037c85bc55b5589105eadaa7634bf846103bfcab424b52d0e30ff4b230`
- Contract SHA-256: `4e7583fb843a1faac177ea14d6a7f6e4e61363178228fbf6a1c60858bbd0c462`

The zero-exit local pipe passed locked offline metadata, formatting, debug and
release all-target/all-feature suites, doctests, support-matrix tests, privacy
regression, vertical slice (including the macOS Keychain refusal path), Clippy,
reproducibility, the sandboxed offline-network canary, docs, ShellCheck,
actionlint, roadmap, fixture, reproducibility, task-digest, and diff checks.
`cargo-audit` and `cargo-deny` remain unavailable on this device; no hosted result
is substituted. The final decision is unchanged: macOS 26 arm64 is verified on
this device; macOS 15 and Intel remain explicit no-go/unverified release gates.
