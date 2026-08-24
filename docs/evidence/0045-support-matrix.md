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

After this branch is reviewed and merged, rerun the exact matrix test and full
device pipe against the merged `main` SHA, then append the merged SHA, log,
commands, artifact digests, limitations, and final decision here.
No issue closure is valid before that post-merge rerun.
