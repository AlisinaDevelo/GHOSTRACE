# Task 0093 evidence: red-team shell secret leakage

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#321](https://github.com/AlisinaDevelo/GHOSTRACE/pull/321)
was squash-merged to protected `main` at
`489563e8106a66f206f40ba5fa0ccd0c7ae7cef5`. This task hardens the explicit
shell-wrapper boundary with a synthetic negative corpus; it does not ship a shell
executor, PTY, ambient terminal collector, or command capture path.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0093-01 | The corpus covers tokens in arguments, environment, stdin, stdout, stderr, executable names, working paths, and failure messages. | [`fixtures/shell-secret-leakage-v1.json`](../../fixtures/shell-secret-leakage-v1.json) contains 13 ordered channels: arguments, environment, stdin, stdout, stderr, executable name, working path, failure message, prompt, process title, diagnostic, crash-report context, and command text. Every row has a unique deterministic synthetic sentinel; 11 rows expect rejection and the two operating-system rows are explicitly `os_visible_not_retained`. |
| E-0093-02 | Journal, logs, errors, exports, panic output, and process inspection are checked for unique sentinels. | `tests/shell_secret_leakage.rs` has six tests. The metadata test injects each sentinel into the strict schema; the journal test drives all 13 through fixture parsing, journal ingestion, error/debug formatting, SQLite bytes, export failure, diagnostic text, and CLI output; the panic test checks child panic output; the macOS process test checks `/bin/ps` exposure and a path-free retained summary. All six pass on the merged macOS device. |
| E-0093-03 | Any unavoidable operating-system exposure is documented separately from GHOSTRACE retention claims. | The fixture’s `process_inspection` and `crash_report` external-exposure rows state why an OS inspector or crash reporter may observe synthetic process state and set `retained_by_ghostrace: false`. Architecture, privacy, event-model, evaluation, and README docs repeat that this is not an OS-hiding guarantee. Non-macOS runners emit an explicit no-go because the macOS inspector contract is unavailable. |

## Delivery

- Issue: [#97](https://github.com/AlisinaDevelo/GHOSTRACE/issues/97)
- Implementation PR: [#321](https://github.com/AlisinaDevelo/GHOSTRACE/pull/321)
- Implementation commits before squash: `3e1054a8ed974a42d41043f529268923f8f2a26d`, `30859ab`
- Protected-main merge: `489563e8106a66f206f40ba5fa0ccd0c7ae7cef5`
- Verification date: 2026-08-26 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
merged source revision: 489563e8106a66f206f40ba5fa0ccd0c7ae7cef5
```

## Merged-main device verification

Every command in this section ran from the exact protected-main SHA above. Hosted
checks are corroboration; the retained device logs are the acceptance evidence.

### Deterministic, privacy, failure, and recovery lanes

- `CARGO_BUILD_JOBS=1 scripts/reproducibility-test.sh` exited `0`. It checked the
  20-fixture manifest, schema/golden comparisons, shell lifecycle 7/7, shell
  secret-leakage 6/6, deterministic demo/journal/export/retention/integrity/
  authenticated-state/recovery flows, capture refusal, 46 Python tests, rustfmt,
  Clippy with `-D warnings`, and all debug targets/features. The script skips only
  its separately authorized native filesystem benchmark.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `cargo +1.88.0 test --release --locked --test shell_secret_leakage -- --nocapture`
  exited `0`; all six tests passed in optimized mode.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 test --release --locked --all-targets --all-features
  -- --skip macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt`
  exited `0`; every non-benchmark target passed, including the Linux-compatible
  platform no-go test and macOS shell leakage test.
- The merged `scripts/offline-network-test.sh` lane passed its network-denial
  canary, privacy regression, and complete product suite before reaching the
  existing native filesystem benchmark. That benchmark failed its unchanged
  30-second per-scenario bound after 168.09s (exit `101`,
  `scenario exceeded bounded run time`). This is an explicit resource no-go; no
  limit was weakened and no result was reported as a pass.

### Direct merged-main shell receipts

The optimized full shell leakage command exited `0` with six tests passed. The
macOS-only process inspection assertion was also run alone from the merged SHA:

```text
cargo +1.88.0 test --release --locked --test shell_secret_leakage \
  -- --exact unix::process_inspection_exposure_is_external_and_not_retained --nocapture
```

It exited `0` with one test passed. The test starts a synthetic process whose
argument sentinel is visible to `/bin/ps`, then asserts the GHOSTRACE retained
summary contains no sentinel. The panic helper similarly verifies that an
untrusted crash sentinel is absent from child stdout and stderr. On Linux, the
same test binary records an explicit no-go rather than assuming `/bin/ps` exists.

## Hosted review and protected merge

PR #321 was pushed from `feature/shell-secret-leakage` and merged only after both
duplicate push/PR check runs were green: audit, Clippy, deny, dependency review,
roadmap, rustfmt, offline fixture/network denial, Linux stable, Linux MSRV, and
macOS stable. The first run exposed a genuine portability issue because the Linux
container has no `/bin/ps`; commit `30859ab` gates that assertion to macOS and
adds the explicit non-macOS no-go. The rerun passed all checks; no test bound was
relaxed.

## Retained artifacts

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `/tmp/ghostrace-0093-merged-device-info.txt` | exact device/toolchain capture | `171d47acfcb406e79f29fa60f38021d4a21825529af6fbad29b962dd2319d85a` | 869 |
| `/tmp/ghostrace-0093-merged-repro.log` | merged-main deterministic pipe, exit 0 | `de61994c352264b7a535f0fdc9c70f43838f96e3ab041b6e0467266b2dc17e99` | 40704 |
| `/tmp/ghostrace-0093-merged-release-build.log` | optimized release build, exit 0 | `da8b5fa2bfff898291a3104b9431e7497d8100c2edaed86ee56c03a33dc6a15d` | 3430 |
| `/tmp/ghostrace-0093-merged-release-secret-exact.log` | optimized leakage suite, 6/6, exit 0 | `efe500d4c5d13cc18654f33f9beec3048db811a3cc8fe67c656cf8d6805606b0` | 697 |
| `/tmp/ghostrace-0093-merged-rustdoc.log` | rustdoc, exit 0 | `955ac5f46ccf44744d35eb9b53ecd5987702d0800327360ab427b6effac0fd52` | 630 |
| `/tmp/ghostrace-0093-merged-release-all.log` | optimized all-target/all-feature matrix excluding benchmark, exit 0 | `891382d111ef29b07f506524d098d7f1d7dde2fb262f3f90e7a90913088d2b20` | 30231 |
| `/tmp/ghostrace-0093-merged-process-inspection.log` | macOS process exposure assertion, 1/1, exit 0 | `318d9e77866ea0be766867647848af5e1d502bfee37a32f8564e329aa6acaa62` | 352 |
| `/tmp/ghostrace-0093-merged-contracts.log` | manifest, reproducibility, roadmap, Python tests, rustfmt, exit 0 | `8dd8528b91522338cc920bb870dfcc77af0aa98bed199a1ebb7154d3549d5389` | 1281 |
| `/tmp/ghostrace-0093-merged-offline.log` | network/privacy/product pass; native benchmark no-go, exit 101 | `2f1fde4738298fcf360a9843779160b6370a4ebd9cdb6b6ca1bca1cd94bd876e` | 12977 |
| `fixtures/shell-secret-leakage-v1.json` | deterministic fixture registered in manifest | `a86e12ca24c01e9cb08bad0c8f2b25a50c750fdc08d7113d6ace83f5f3b6a512` | 3441 |

## Privacy, failure, recovery, and scope boundaries

- All sentinels are synthetic and deterministic. The fixture is marked
  `user_data_included: false`, `network_required: false`, `captures_stdio: false`,
  `captures_environment: false`, and `retains_command_text: false`.
- Metadata deserialization is deny-unknown-fields and semantic validation rejects
  every injected channel without echoing its value. Invalid fixture ingestion
  leaves the journal empty; export failure does not publish a destination; raw
  SQLite bytes and diagnostic strings contain no sentinel.
- Panic output is checked in a child process with a cleared environment. Process
  inspection and crash-report context are not application-retention channels and
  are explicitly documented as external OS exposure; GHOSTRACE makes no claim to
  hide synthetic values from the operating system.
- This task adds no event kind or field and does not implement a shell executor,
  PTY, terminal-close detector, process attribution, production key management,
  or a policy/consent gate. Those remain parent task 0024 work.
