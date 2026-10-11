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

Install `nightly-2026-10-03` and `cargo-fuzz` 0.13.2, then fetch the locked
test dependencies before running the offline wrapper:

```text
cargo +nightly-2026-10-03 fetch --locked --manifest-path fuzz/Cargo.toml
sh fuzz/run-bounded.sh
```

The wrapper defaults to one 60-second run per target, one libFuzzer worker,
2-second individual-input timeout, a 256 MiB RSS limit, and a 64 KiB maximum
input. `frame_decoder` uses its explicit 131080-byte decoder-buffer bound.
Override `FUZZ_SECONDS`, `FUZZ_TIMEOUT_SECS`, `FUZZ_RSS_MB`, and
`FUZZ_MAX_LEN` only when the receipt records the changed values. The script
replays every checked-in seed and optional `fuzz/regressions/<target>/` input
before each campaign. It keeps bounded logs, per-target receipts, failure
inputs and a copy of each campaign corpus below ignored `fuzz/artifacts/`;
mutation never changes the checked-in seeds. It selects the Rust compiler's
host target, not the architecture of the `cargo-fuzz` tool binary.
`FUZZ_TARGET_TRIPLE` and `FUZZ_BUILD_DIR` can override the target and build-cache
path. The wrapper records both, exact commands, compiler identity, input and
log digests, CPU and wall time, execution counts, peak RSS, and exit outcomes.
It refuses existing evidence directories and output inside checked-in inputs.
Builds have a 900-second deadline; replay and campaign subprocess groups have
outer deadlines and a 4 MiB log cap. Limit overrides are validated and recorded.
Interrupted subprocesses retain an interrupted receipt and stop the run.

`.github/workflows/native-fuzz.yml` runs the same seven targets on Linux for
pushes, pull requests, merge queues and manual dispatch. It uses pinned tools,
serial builds, 900-second build deadlines, a 45-minute job limit and uploads
synthetic receipts/logs/corpora for 30 days, including on failure. The Python
runner regressions use a simulated compiler boundary and do not substitute
for the actual Rust campaigns.

The initial local run of this candidate was interrupted during its first target;
it is incomplete. Building a fuzz crate or replaying seeds is not a campaign.
Completed runs must be identified by their exact source revision and receipt.

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

Failure inputs are retained with hashes; the runner does not automatically
minimize them. Minimized regressions must be reviewed and committed under
`fuzz/regressions/<target>/` for subsequent replay. Live Safari integration
and journal/service resource checks remain separate evidence gates.
