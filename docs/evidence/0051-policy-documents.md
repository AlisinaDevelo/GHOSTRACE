# Task 0051 evidence: versioned capture-policy documents

Status: complete.

Task 0051 turns policy history into a strict, versioned document boundary. The
document schema is explicit, unknown inputs fail closed, and migrations cannot
silently change the meaning of consent.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0051-CODE-EDF40DD` — `edf40dd510c6f7093400d01f760f8cb506d71762` |
| Implementation pull request | [#187](https://github.com/AlisinaDevelo/GHOSTRACE/pull/187) |
| Protected-main merge | `f114c1b8d57e0c69f7894fa885667885e08d1c40` |
| Post-merge local pipeline log | `GHOSTRACE-0051-POSTMERGE-LOG-20260824` — SHA-256 `fff6cd802469a519bb6c24e18d1ffe9099149989aab691889f00dcc08f974245` |

The raw verification log is retained locally only. It records tool and test
results but does not publish event payloads, fixture contents, credentials,
account data, network service data, or private host identifiers.

## Acceptance mapping

1. **Versioned schema and golden documents.** `schemas/policy-document-v1.json`
   defines the strict v1 object, canonical fields, bounds, and uniqueness
   constraints. Checked-in valid, unknown-version, unknown-field, and duplicate
   entry fixtures exercise the JSON Schema and Rust parser together.
2. **Safe migrations.** `PolicyDocument` validates programmatic and serde
   construction through one path and converts to the existing runtime
   `PolicyProfile`. `PolicyDocument::migration_from` classifies unchanged
   upgrades as choice-preserving and semantic changes as requiring explicit
   reconfirmation. `PolicyHistory::apply` installs only validated candidates and
   never replaces the active document when reconfirmation is absent.
3. **Fail-closed history.** Duplicate identity/version candidates, downgrade
   attempts, unknown schema versions, duplicate entries, unknown fields, and
   invalid identifiers return bounded errors before insertion. The migration
   regression proves the active version remains unchanged after rejected
   candidates and that only an explicit reconfirmation commits a semantic change.

## Local verification on protected main

Target: the local macOS development environment; no live collector or network
service was enabled. The exact source under test was merge SHA
`f114c1b8d57e0c69f7894fa885667885e08d1c40`.

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, 1 origin unit test, 23 Rust
  integration tests, clippy, and locked all-target tests.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the enforced denial canary and complete product suite.
- `cargo +1.88.0 doc --no-deps` — pass.
- `cargo +1.88.0 test --locked --release --all-targets --all-features` — pass:
  1 origin unit test, 1 privacy regression, 5 support-matrix tests, and 23
  vertical-slice tests.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` —
  pass.
- `shellcheck scripts/offline-network-test.sh`, `python3 scripts/roadmap.py
  check`, `python3 scripts/fixture-manifest.py check`, `python3
  scripts/reproducibility.py check`, and `git diff --check` — pass.

Hosted checks on PR #187 were green and served only as protected-branch merge
gates. They are not acceptance evidence for this task; the post-merge local
pipeline above is the evidence record.
