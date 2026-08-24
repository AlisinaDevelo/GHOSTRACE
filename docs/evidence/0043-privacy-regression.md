# GHOSTRACE 0043 privacy regression evidence

Artifact IDs: `GHOSTRACE-0043-PRIVACY-CORPUS-V1`,
`GHOSTRACE-0043-PREMERGE-AFC0E3B`.

This is the pre-merge target-device record for the prohibited-data regression
corpus. The roadmap task remains open until the same reproduction is rerun
against the merged `main` SHA and linked from the issue.

## Implementation under test

- Source: [`tests/privacy_regression.rs`](../../tests/privacy_regression.rs)
- Corpus manifest: [`tests/fixtures/privacy-regression-v1.json`](../../tests/fixtures/privacy-regression-v1.json)
- Commit: `afc0e3bee2517cb18426df3420ef0229cad81624`
- Source SHA-256: `56f597bb85f76666c50b45d58d28032266f3ae1fc3ff3b9d8ee30a1219d10b4e`
- Manifest SHA-256: `9c83a4c94be3498980f396c265958bf3eee7978edad9f94547fbaa4e46e72bde`

## Device and toolchain

- Device: MacBook Pro 17,1, Apple M1, 8 GB
- OS: macOS 26.6.2 (25G83), arm64
- Kernel: Darwin 25.6.0
- Rust: `rustc 1.88.0`, Cargo 1.88.0
- Corpus cases: credential, environment variable, command argument, window
  title, page content, clipboard text, and private-browser marker

## Target-device reproduction and results

All commands ran locally on the device above at this pre-merge commit:

- `cargo fmt --all -- --check` — pass
- `cargo test --locked --all-targets --all-features` — pass (19 tests)
- `cargo test --locked --release --all-targets --all-features` — pass (19 tests)
- `cargo test --locked --test privacy_regression -- --exact` — pass (1 corpus test)
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — pass
- `actionlint` — pass
- `python3 scripts/roadmap.py check` — pass
- `python3 -m unittest discover -s tests -p 'test_roadmap.py' -v` — pass (23 tests)
- `python3 scripts/roadmap.py index | cmp - .forge/tasks/README.md` — pass

The corpus test injects a unique sentinel for each case and exercises fixture
parsing, journal ingest, explanation lookup, export, and CLI diagnostics. It
asserts that no sentinel is retained, echoed in display/debug errors or
stdout/stderr, written to an export, or persisted in the journal. Failure
output is keyed only by case identifier. The full verification log was scanned
for the sentinel prefix and was clean.

The raw local verification log is identified by SHA-256
`70bbbd6038b37d2ffd72bb6aecc732417a4dc5e4b65d240a884ac2432a899382`.

## Acceptance coverage

- Happy path: existing fixture ingest, deterministic explanation, JSONL export,
  schema, crypto, and CLI tests passed on this device.
- Negative/privacy: all seven manifest cases reject before retention and never
  echo the injected value.
- Failure: invalid schema, spoofed provenance, policy denial, export overwrite,
  and explicit live-capture refusal tests passed.
- Recovery: idempotent migration and SQLite safety checks passed.
- Resource: fixture/event size-limit and bounded-error tests passed.

## Limitations and next gate

The corpus is synthetic and fixture-only. It does not enable live capture,
request macOS permissions, or open a network connection. `cargo-audit` and
`cargo-deny` were not installed locally; their CI jobs remain separate policy
checks and are not substituted for the target-device evidence above. No
hardware is required for this task. The post-merge device rerun and merged
commit must be recorded in the corresponding GitHub issue before closure.
