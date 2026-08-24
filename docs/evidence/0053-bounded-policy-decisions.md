# Task 0053 evidence: bounded policy decisions and refusal reasons

Status: complete.

Task 0053 adds a finite, privacy-bounded decision surface. Policy outcomes are
explicitly allow, deny, redact, summarize, or refuse, while diagnostics classify
accepted decisions, policy denial, malformed input, unsupported scope, and internal
failure without retaining the rejected value.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0053-CODE-875C431` — `875c431921f6c59d8d5ac6380d6f988fb92c0df9` |
| Implementation pull request | [#191](https://github.com/AlisinaDevelo/GHOSTRACE/pull/191) |
| Protected-main merge | `b4ae45fbd5958a79e60da85bff33f06cf128a01b` |
| Post-merge local pipeline log | `GHOSTRACE-0053-POSTMERGE-LOG-20260824` — SHA-256 `eb97c8c9b7b0a20b08ae8b792ea6bd00b7bc3174bb43a5539d0da45d345561cc` |

The raw verification log is retained locally only. It records tool and test
results but does not publish rejected roots, event payloads, credentials, account
data, network service data, or private host identifiers.

## Acceptance mapping

1. **Finite outcomes and bounded metadata.** `PolicyOutcome` defines the five
   allowed dispositions. `PolicyDecisionRecord` carries only source, optional
   validated policy identity, policy version, root presence, private-context state,
   outcome, and a finite `PolicyReason`; it never carries a root string or payload.
   Redact, summarize, and refuse helpers preserve this boundary.
2. **Diagnostic separation.** `PolicyDiagnostic` distinguishes accepted, policy
   denied, malformed input, unsupported scope, and internal failure. The reason
   registry maps every public reason to one of those finite classes, including
   invalid profiles and explicit refusal dispositions.
3. **Adversarial privacy coverage.** The vertical slice passes a path containing
   private/secret components through both decision and debug paths, then asserts
   that neither serialized records nor `Debug` output echo it. Invalid policy
   identities are refused without echoing the candidate.

## Local verification on protected main

Target: the local macOS development environment; no live collector or network
service was enabled. The exact source under test was merge SHA
`b4ae45fbd5958a79e60da85bff33f06cf128a01b`.

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, 1 origin unit test, 25 Rust
  integration tests, clippy, and locked all-target tests.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the enforced denial canary and complete product suite.
- `cargo +1.88.0 doc --no-deps` — pass.
- `cargo +1.88.0 test --locked --release --all-targets --all-features` — pass:
  1 origin unit test, 1 privacy regression, 5 support-matrix tests, and 25
  vertical-slice tests.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` —
  pass.
- `shellcheck scripts/*.sh`, `python3 scripts/roadmap.py check`, `python3
  scripts/fixture-manifest.py check`, `python3 scripts/reproducibility.py
  check`, and `git diff --check` — pass.

Hosted checks on PR #191 were green and served only as protected-branch merge
gates. They are not acceptance evidence for this task; the post-merge local
pipeline above is the evidence record.
