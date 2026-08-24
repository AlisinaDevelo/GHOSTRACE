# Task 0050 evidence: semantic wrappers for retained fields

Status: complete.

Task 0050 makes the event model's privacy and canonical-encoding rules
constructible invariants. The wrappers preserve the existing string JSON wire
format while preventing invalid retained values from being built or decoded
into payloads and envelope metadata.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0050-CODE-6CF41B7` — `6cf41b7696cea16f5cf3c4c56253aaadc65fe6d6` |
| Implementation pull request | [#185](https://github.com/AlisinaDevelo/GHOSTRACE/pull/185) |
| Protected-main merge | `96213c141c5e5603d4f4367566567c8fccd340d7` |
| Post-merge local pipeline log | `GHOSTRACE-0050-POSTMERGE-LOG-20260824` — SHA-256 `8c0363330447267ed58de99ff5b1bb4707928010c7f42b168cee8fdcb7dd079a` |

The raw verification log is retained locally only. It records tool and test
results but does not publish event payloads, fixture contents, credentials,
account data, network service data, or private host identifiers.

## Acceptance mapping

1. **Canonical, bounded, privacy-safe wrappers.** Distinct fallible wrappers
   cover opaque IDs (including roots, repositories, sessions, shells, browsers,
   bookmarks, folders, collector instances, lifecycle labels, provenance, and
   policy IDs), application IDs, Git branches and object IDs, SHA-256 digests,
   source cursors, and reason codes. Constructors enforce the existing ASCII,
   byte-length, grammar, digest, and forbidden-content-sentinel rules without
   echoing rejected values.
2. **One construction and deserialization path.** Every wrapper's `new`,
   `TryFrom`, and `FromStr` path calls its validator. Its `Deserialize`
   implementation calls `TryFrom`, so JSON payloads and envelope metadata use
   exactly the same checks as programmatic construction. Serialization remains
   a bounded string, preserving event schema compatibility.
3. **Mutation/property coverage.** The
   `semantic_wrappers_reject_mutations_before_serialization` regression mutates
   canonical values with traversal, control, Unicode, credential, and forbidden
   sentinel forms; each form is rejected by construction and serde. It also
   round-trips valid wrappers and exercises object IDs, branches, cursors,
   digests, application IDs, bookmarks, folders, and reason codes.

## Local verification on protected main

Target: the local macOS development environment; no live collector or network
service was enabled. The exact source under test was merge SHA
`96213c141c5e5603d4f4367566567c8fccd340d7`.

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, 1 origin unit test, 22 Rust
  integration tests, clippy, and locked all-target tests.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the enforced denial canary and complete product suite.
- `cargo +1.88.0 doc --no-deps` — pass.
- `cargo +1.88.0 test --locked --release --all-targets --all-features` — pass:
  1 origin unit test, 1 privacy regression, 5 support-matrix tests, and 22
  vertical-slice tests.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` —
  pass.
- `shellcheck scripts/offline-network-test.sh`, `python3 scripts/roadmap.py
  check`, `python3 scripts/fixture-manifest.py check`, `python3
  scripts/reproducibility.py check`, and `git diff --check` — pass.

Hosted checks on PR #185 were green and served only as protected-branch merge
gates. They are not acceptance evidence for this task; the post-merge local
pipeline above is the evidence record.
