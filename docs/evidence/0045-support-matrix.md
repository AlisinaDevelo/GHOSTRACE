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
- Source commit: `PENDING_AFTER_COMMIT`
- Contract SHA-256: `PENDING_AFTER_COMMIT`

## Device and limitation

- Device: MacBook Pro 17,1, Apple M1, 8 GB
- OS: macOS 26.6.2 (25G83), arm64
- Rust/Cargo: 1.88.0
- Verified row: macOS 26 arm64
- No-go rows: Intel (`x86_64`) hardware was not available
- Still unverified: the macOS 15 Sequoia floor on both target architectures

No Actions result is used as evidence. The release gate must retain a separate
beta/release-candidate run for the floor and every target architecture.

## Acceptance map

| Acceptance criterion | Retained evidence |
| --- | --- |
| macOS major versions and Intel/Apple-silicon expectations are explicit | `support-matrix-v1.json` `support_floor`, `architectures`, and `platform_matrix` |
| Every collector declares required, optional, prohibited permissions and refusal behavior | `support-matrix-v1.json` `collectors[*].permissions`; `cargo test --test support_matrix` |
| Annual beta/RC validation has owner, evidence format, and retirement rule | `support-matrix-v1.json` `annual_validation`; `cargo test --test support_matrix` |

## Required rerun record

After this branch is reviewed and merged, rerun the exact matrix test and full
device pipe against the merged `main` SHA, then replace the pending source/digest
fields and link the merged log, commands, artifact digests, and final decision.
No issue closure is valid before that post-merge rerun.
