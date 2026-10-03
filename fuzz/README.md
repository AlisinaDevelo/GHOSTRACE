# GHOSTRACE native-boundary fuzzing

This directory is an authentic `cargo-fuzz`/libFuzzer package for the
untrusted extension-to-host boundary. Every target calls production Rust code
and has a checked-in seed corpus. The package is deliberately offline: it does
not launch a browser, open a network listener, access the keychain, or write a
journal.

Targets:

- `frame_decoder` — native-endian length prefixes, truncation, and decoder
  buffering.
- `schema_parser` — bounded JSON syntax, fields, nesting, strings, and typed
  message deserialization.
- `origin_validation` — Chromium caller-origin and bounded Safari identity
  validation.
- `pairing_state` — pairing identity, digest, revocation, and admission
  decisions against a fixed synthetic approved record, with exact refusal
  oracles and cross-session/body/sequence MAC rejection assertions.
- `sequence_handling` — hello-first, sequence replay/gap handling, ingress
  rate admission, and idle deadline state, with independent acceptance, gap,
  replay, timeout and shutdown assertions.
- `policy_conversion` — hostile URL conversion and policy decision records.
- `transport_differential` — equal protocol bodies through the pure Chromium
  frame and Safari-shaped envelope fixtures.

The Safari-shaped code is a transport fixture, not a shipped Safari adapter.
It intentionally proves only normalization of an explicitly identified,
bounded envelope. Live Safari extension packaging, permission grants, app
extension messaging, and device parity remain separate acceptance gates.

## Bounded campaign

After the Rust toolchain and `cargo-fuzz` are available, run one target at a
time with the repository's `fuzz/run-bounded.sh` wrapper:

```text
bash fuzz/run-bounded.sh
```

The wrapper defaults to one 60-second run per target, one libFuzzer worker,
2-second individual-input timeout, a 256 MiB RSS limit, and a 64 KiB maximum
input. `frame_decoder` uses its explicit 131080-byte decoder-buffer bound.
Override `FUZZ_SECONDS`, `FUZZ_TIMEOUT_SECS`, `FUZZ_RSS_MB`, and
`FUZZ_MAX_LEN` only when the receipt records the changed values. The script
keeps logs and a copy of each campaign corpus below ignored `fuzz/artifacts/`;
mutation never changes the checked-in seeds. It selects the Rust compiler's
host target, not the architecture of the `cargo-fuzz` tool binary.
`FUZZ_TARGET_TRIPLE` and `FUZZ_BUILD_DIR` can override the target and build-cache
path; record both in the receipt. Use an installed nightly Rust toolchain (for
example through `RUSTUP_TOOLCHAIN`) and record its exact version.

The campaign has not been run for this candidate branch. Building a fuzz crate
is not a campaign and is not reported as one.

Dependencies are pinned by `fuzz/Cargo.lock`. Check the unpublished harness
against the production policy with `cd fuzz && cargo deny --locked --offline
--config ../deny.toml check bans licenses sources`. The crate/version-scoped
NCSA exception covers only `libfuzzer-sys` 0.4.13; production allowances are
unchanged.

## Required receipt

A closure-quality run retains, for every target, the source commit, device and
toolchain, exact command-line flags, wall time, executions, peak RSS, timeout
and crash/hang outcome, and SHA-256 checksums of any minimized regression
inputs. Store only synthetic inputs in the repository; redact or discard any
secret-bearing diagnostic. A zero-crash result without the per-target receipt
does not close task 0109.
