# GHOSTRACE 0043 privacy regression evidence

Artifact IDs: `GHOSTRACE-0043-PRIVACY-CORPUS-V1`,
`GHOSTRACE-0043-PREMERGE-AFC0E3B`, `GHOSTRACE-0043-MERGED-AC7B4CC`,
`GHOSTRACE-0043-LOCAL-PIPELINE-98420EE`, `GHOSTRACE-0043-LOCAL-AUDIT`,
`GHOSTRACE-0043-LOCAL-DENY`, `GHOSTRACE-0043-LOCAL-NETWORK`, and
`GHOSTRACE-0043-MERGED-LOCAL-20260824`.

This is the retained target-device record for the prohibited-data regression
corpus. The same reproduction has been rerun against merged `main`; the roadmap
task and issue remain open only until the evidence ledger is synchronized.

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

## Merged-main reproduction

The same reproduction was rerun on protected `main` at merged commit
`ac7b4cc878ffadbcf43ccbaec15c99b7588b4226` on the same device and toolchain.
The debug and release suites each passed 19 tests, the focused corpus test
passed, Clippy, formatting, actionlint, and all roadmap checks passed, and the
sentinel scan was clean. The merged-run log is identified by SHA-256
`f3886a325650b3be32670875802509f45697259878ff8c9bc51c2154f210a2c3`.

## Expanded local pipeline (Actions not used as evidence)

At commit `98420ee654ebeddb70f11bed9a3733a9f74b1d60`, the full pipeline was
rerun locally with `CARGO_NET_OFFLINE=true`: locked metadata, check, build,
two full debug suites, release suite, five repeated privacy runs, doctests,
Clippy, formatting, actionlint, roadmap tests/index parity, CLI help/version,
deterministic demo, export permissions/overwrite/force, schema equality, and
explicit capture refusal. Every command passed; the local sentinel scan was
clean. The local-pipeline log digest is
`cca66853751602af4d1db0d14d85bf5f07e1ebec7a75542abe4b3b3c087f6b63`.

The local RustSec audit used `cargo-audit 0.22.2` and scanned 178 locked
dependencies with no advisories; log digest
`bdbcfb9e01bfd07f6b14cb986421667eec10a2da27e230821584ec21631e8403`.
The local policy check used `cargo-deny 0.20.2` and passed advisories, bans,
licenses, and sources; it emitted only duplicate-dependency and unmatched
license-allowance warnings; log digest
`2f1e8510d523fa397202e9f6938b1f7da99ed8755e954710df9d412adf239025`.
The dependency/source network-surface scan found no network-client dependency
or network API reference; log digest
`df3e48b3dd76e3e4944d49ce121ba320fe85b74b717cadd934be42e6c82f9030`.

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
request macOS permissions, or open a network connection. The local audit and
policy tools were installed into task-scoped temporary paths and passed; their
logs and digests are recorded above. No hardware is required for this task.
The corresponding GitHub issue must link these artifacts, logs, digests,
limitations, and the merged commit before closure.

## Latest merged-main device rerun

The complete local pipe was rerun against merged `main` commit
`b0442e91cdfd449f59bdc762ea550346ffd18a0b` on the same MacBook Pro M1 using
macOS 26.6.2 arm64, Rust/Cargo 1.88.0, and no GitHub Actions evidence. The
retained log is `/tmp/ghostrace-main-device-corrected-20260824-095638.log` with
SHA-256 `560548b20f1ccf23467cb4595f949d769a3cd7cc6a1105470d49a13462cba279`.

The run passed formatting, debug and release all-target/feature tests (19 each),
the focused privacy corpus, Clippy, actionlint, roadmap validation and 23 Python
roadmap tests, generated-index parity, schema/demo/export, export overwrite
refusal and `--force` recovery, and explicit live-capture refusal. `cargo-audit`
and `cargo-deny` were not installed on this device and are recorded as
unavailable rather than replaced by CI claims.
