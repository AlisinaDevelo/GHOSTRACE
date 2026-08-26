# Task 0091 evidence: explicit shell metadata schema

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#316](https://github.com/AlisinaDevelo/GHOSTRACE/pull/316)
was squash-merged to protected `main` at
`014791b57dfe1baa646f7aaf08cd73b661a7214c`. This task defines a metadata-only
contract; it does not add a shell executor or ambient shell capture.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0091-01 | The schema permits executable identity, sanitized working-directory identity, start/end time, exit status, signal, and wrapper session only. | [`schemas/shell-execution-metadata-v1.json`](../../schemas/shell-execution-metadata-v1.json) is strict draft 2020-12 JSON Schema with `additionalProperties: false`, and [`fixtures/shell-execution-metadata-v1.golden.json`](../../fixtures/shell-execution-metadata-v1.golden.json) is a valid v1 record. `ShellExecutionMetadata` and `ShellWorkingDirectory` in `src/shell_metadata.rs` expose only the version, wrapper session, normalized executable token, path class/digest, timestamps, outcome, exit code, and signal. The four outcome combinations are validated in `tests/shell_metadata.rs::valid_outcomes_round_trip_and_preserve_only_allowed_fields`. |
| E-0091-02 | Arguments, environment, standard input, output, shell history, aliases, and expanded command text are structurally impossible to retain. | The root and nested wire structs deny unknown fields; no prohibited field exists in the Rust types or schema. `ShellExecutableId` accepts only a bounded lowercase opaque token and rejects path separators and credential-like sentinels. The 18-case [`tests/fixtures/shell-metadata-adversarial-v1.json`](../../tests/fixtures/shell-metadata-adversarial-v1.json) covers arguments/argv, environment/env, stdin/stdout/output, shell history, aliases, command/expanded text, raw paths, credential-like executable identity, and inconsistent outcome fields. `tests/shell_metadata.rs::adversarial_fixture_rejects_secrets_and_raw_shell_state_without_echo` requires schema and typed rejection without echoing sentinels or raw paths. |
| E-0091-03 | Every field has semantic validation, sensitivity classification, and adversarial fixtures. | `SHELL_METADATA_FIELDS` and the schema's `x-ghostrace-field-classification` extension enumerate all 10 retained fields with semantic and sensitivity classes. Tests compare the registry and schema byte-for-byte, validate the golden, reject reversed timestamps, invalid digest/outcome/signal combinations, credential-like identities, unknown fields, and the 16 KiB record bound, and exercise all 18 adversarial fixtures. `fixtures/manifest.json` binds the golden and adversarial fixture hashes. |

## Delivery

- Issue: [#95](https://github.com/AlisinaDevelo/GHOSTRACE/issues/95)
- Implementation PR: [#316](https://github.com/AlisinaDevelo/GHOSTRACE/pull/316)
- Implementation commit before squash: `b49e996ee4c57350962a47595e8f033f36affb7a`
- Protected-main merge: `014791b57dfe1baa646f7aaf08cd73b661a7214c`
- Verification date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2
MacBookPro17,1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
merged source revision: 014791b57dfe1baa646f7aaf08cd73b661a7214c
```

## Merged-main device verification

Every command in this section ran from the exact protected-main SHA above.
Hosted checks are corroboration; the retained device logs are the acceptance
evidence.

### Deterministic, privacy, failure, and recovery lanes

- `CARGO_BUILD_JOBS=1 scripts/reproducibility-test.sh` exited `0`: the pinned
  18-fixture manifest, all schema/golden comparisons (including `shell-schema`),
  deterministic demo/journal/export/retention/integrity/authenticated-state/
  recovery flows, 46 Python checks, rustfmt, Clippy with `-D warnings`, and all
  Rust targets with the separately invoked native resource test omitted.
- `cargo +1.88.0 build --locked --release` exited `0`.
- `cargo +1.88.0 test --locked --release --test shell_metadata -- --nocapture`
  exited `0`; 4/4 shell metadata tests passed in optimized mode.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `CARGO_BUILD_JOBS=1 cargo +1.88.0 test --locked --release --all-targets
  --all-features -- --test-threads=1` exited `0`; every test target passed,
  including the native benchmark and its explicit `cursor_regression` gap
  reporting. One Keychain authorization test and the 10-million-record stress
  test remain intentionally ignored by their existing device-authorization/
  resource gates.

The local sandboxed debug offline lane also ran the network-denial canary and
privacy/product tests. Its unrelated debug native filesystem benchmark exceeded
the existing 30-second per-scenario bound (176.60s in the full lane and 120.24s
when isolated); this is retained as a no-go rather than hidden or “fixed” in
0091. The optimized merged-main run passed the same native benchmark, so the
release receipt below is the reproducible device result.

### Native device resource receipt

The direct merged-main command was run on the named Mac:

```text
CARGO_BUILD_JOBS=1 cargo +1.88.0 test --locked --release --test filesystem_benchmark -- --nocapture --test-threads=1
```

It exited `0` after `32.61s`; both tests passed across all 24 synthetic
scenario runs. The receipt recorded maximum latency `8430.295375ms`, CPU
user/system `24823.541/1040.642ms`, disk growth `6138464` bytes, energy
`864020nJ`, and RSS peak `16728064` bytes. The event-storm scenarios surfaced
`cursor_regression` as explicit errors/gaps; no unsupported completeness claim
was made.

## Hosted review and protected merge

PR #316 was pushed from `feature/shell-metadata-schema` and merged only after
both duplicate workflow runs were green. All 20 live checks passed: Linux
stable, Linux MSRV, macOS stable, rustfmt, Clippy, roadmap, Cargo policy/deny,
advisories/audit, dependency review, and both network-denial fixture lanes.
Required checks were not bypassed. Evidence PR [#317](https://github.com/AlisinaDevelo/GHOSTRACE/pull/317)
was subsequently reviewed and squash-merged to protected `main` at
`3f62237aeabbdab04433f4fc4cc08ba58f78ebf5`.

## Retained artifacts

| Artifact | Result | SHA-256 |
|---|---|---|
| `/tmp/ghostrace-0091-device-info.txt` | exact device/toolchain capture | `53aa8e12c29bbe4d11d38323498bafe378be2ec8a7aa68aeca02d94604e08c3e` |
| `/tmp/ghostrace-0091-merged-repro.log` | merged-main deterministic pipe, exit 0 | `649dc4dcbc198a2b197cdf1f52dbc2e13be1a44c40a1179f3284eee91afb6b6f` |
| `/tmp/ghostrace-0091-merged-release-build.log` | optimized release build, exit 0 | `597cb739ac4764e9b7dbcf3314ffca896463f1a163d5b6c3ef1a34834c9ebc6e` |
| `/tmp/ghostrace-0091-merged-shell-release.log` | optimized shell tests, 4/4, exit 0 | `5b728c90987a9a98258851d2218ca7581c13909387d00323aafeacb526112380` |
| `/tmp/ghostrace-0091-merged-doc.log` | rustdoc with warnings denied, exit 0 | `2b5d873eb002b2476301e1c4a46745fe41579189e62aeb177b9935c634fc28ab` |
| `/tmp/ghostrace-0091-merged-filesystem-native.log` | optimized native benchmark, 2/2, exit 0 | `3263b56a4214e8a74733a126154ade3bcb300a2473591a16a1e8043358738c1f` |
| `/tmp/ghostrace-0091-merged-release-all-tests.log` | complete optimized all-target/all-feature matrix, exit 0 | `f6f9cee5c2637bed0e6f08a4c809f77c6615f6a9eaa54129d5709842c49dc` |
| `/tmp/ghostrace-0091-repro-final.log` | premerge deterministic pipe, exit 0 | `5de0b88fd8b55a3c4afb5ab10c21acb9e6f13eb481e123b240e1b59bc4b099a1` |
| `/tmp/ghostrace-0091-filesystem-native.log` | premerge debug native no-go, bound failure | `ecd361db10a8c60553c822da5f2ea8fbbfb9d930f6866a784a61c502c640bd78` |
| `/tmp/ghostrace-0091-offline-local.log` | sandbox denial canary/privacy pass; debug native bound failure | `5f5b26646afc974cd7d5e61d407b50b8afb38edc643d8bc868d340977678a0c8` |

## Privacy, failure, and scope boundaries

- `ghostrace shell-schema` prints and validates the checked-in contract; it
  never executes a shell and never enables ambient capture.
- The contract does not represent arguments, environment, stdin/stdout, shell
  history, aliases, raw or expanded command text, or raw working-directory
  paths. Invalid metadata fails closed without echoing the rejected value.
- JSON Schema enforces shape and formats; Rust semantic validation additionally
  enforces timestamp ordering, the seven-day wrapper bound, digest identity
  shape, outcome/exit/signal consistency, executable token policy, and the
  16 KiB input bound.
- The working-directory digest is root-scoped metadata supplied by a future
  explicit wrapper; this task does not claim to derive or verify a live root.
- A future wrapper, consent/policy gate, and process attribution remain task
  0024 work. This schema is not evidence of command intent or complete process
  history.
