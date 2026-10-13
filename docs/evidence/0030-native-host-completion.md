# Native host, protocol, and pairing evidence

Observed 2026-10-13 for tasks 0030/#34, 0103/#107 and 0104/#108 on clean protected-main source `ee4216c8fd3ef993538655ac672aa2d37825f6d0`. The native, pairing-process and full fuzz reproductions passed on the target device. The retained delivery receipt binds independent review and passing hosted checks to the merged implementations. Tasks 0103 and 0104 meet their criteria; task 0030 also has its verified dependencies and capstone proof.

The native scope is the explicit pairing CLI and actual native-host executable, authenticated private Unix IPC, and a synthetic fixture journal. Chromium/Safari differential tests are pure transport fixtures. No installed browser extension or live Safari runtime was exercised.

## Source and target

| Record | Source / identity |
|---|---|
| Reconnect fix | [PR #430](https://github.com/AlisinaDevelo/GHOSTRACE/pull/430), merged `f3fe7c1a7fa9c9f8b6124cdfca38532ecc5cb5ed`; reconnect recovery, reproduced by R-NATIVE-01 on the combined merged source |
| Fuzz runner and CI | [PR #433](https://github.com/AlisinaDevelo/GHOSTRACE/pull/433), merged `1f78a3aa92eaee63fdd7012bf69d0b978499ccc6` |
| CI reproducibility fix | [PR #431](https://github.com/AlisinaDevelo/GHOSTRACE/pull/431), merged `b2f86a20bd7ee3b8e02dc98237a2218a1e1dd8f8`; matching action pins and `PYTHONDONTWRITEBYTECODE` environment |
| Pairing process tests | [PR #434](https://github.com/AlisinaDevelo/GHOSTRACE/pull/434), reviewed head `a9ca111ced5f1ddd9b96e96b3c5edc7e107da16e`, merged `ee4216c8fd3ef993538655ac672aa2d37825f6d0`; R-PAIRING-01 |
| Final combined native source | `ee4216c8fd3ef993538655ac672aa2d37825f6d0`, clean; R-NATIVE-01/02, R-PAIRING-01 and R-FUZZ-60-M5-ee4216c; review/check/merge binding R-DELIVERY-01 |
| Target device | `Mac17,3`, Apple M5, `arm64`, macOS `27.0.1` build `26A434`; receipt kernel release `Darwin 27.0.0` |
| Native test toolchain | Rust `1.88.0`, commit `6b00bc3880198600130e1cf62b8f8a93494488cc` (2025-06-23), LLVM `20.1.5`; Cargo `1.88.0` (`873a06493`, 2025-05-10), host `aarch64-apple-darwin` |
| Observed fuzz toolchain | `nightly-2026-10-03`, rustc `1.101.0-nightly` commit `0abfedbc7cd4e725f126913880c95800394f7c37` (2026-10-02), LLVM `23.1.1`, cargo-fuzz `0.13.2`, target `aarch64-apple-darwin` |
| Observed locked inputs | `Cargo.lock` SHA-256 `1f3be4d008439136a36bc23d868702dd42ed1306b8bb89d2bc51a045485eb24a`; `fuzz/Cargo.lock` SHA-256 `7a9cbd2b9c9342e409a894e7707dbf209c82c4d9449996f84befe65bec437770` |

[CONTRIBUTING.md](../../CONTRIBUTING.md#completion-evidence-contract) requires retained criterion-linked results on the exact reviewed and merged source, including target-device happy, negative/privacy, failure, recovery and resource checks. [Roadmap sequencing](../ROADMAP.md#sequencing-and-control-rules) keeps the capstone dependent on verified child deliverables.

## Acceptance map

Evidence IDs remain stable; each supporting observation retains its own source, scope and digest. `R-NATIVE-01` covers focused integration tests, `R-NATIVE-02` bridge unit bounds, `R-PAIRING-01` the four process lifecycle tests, and `R-DELIVERY-01` final review/check/merge receipts. All four receipts are retained. Every criterion below was reproduced on the combined source after PR #434 merged; E-0103-03 additionally uses the seven-target post-merge fuzz campaign.

| Criterion ID | Acceptance criterion | Named proof / supporting receipt |
|---|---|---|
| E-0030-01 | Only paired extension IDs accepted | `unpaired_hello_and_unauthenticated_navigation_never_reach_the_sink`, `caller_origin_must_match_the_hello_extension_before_pairing`, actual-binary mismatched hello, and independent service pairing-proof refusal; R-NATIVE-01 |
| E-0030-02 | Bounded, schema-validated messages | Frame/structure/field/string/capacity, stdio truncation/deadline/shutdown, ingress accounting, lock and stalled-output bounds; R-NATIVE-01/02 |
| E-0030-03 | Malformed input cannot execute commands or escape journal paths | `malformed_commands_paths_and_oversized_messages_have_no_sink_or_file_effect`, strict path-retention refusal, actual binary-to-service durable origin-only journal projection; R-NATIVE-01 |
| E-0030-04 | Revocable pairing | Active and persisted revocation, concurrent store updates, in-flight fence, timeout-held lease and separate-process CLI revocation; R-NATIVE-01/02, R-PAIRING-01 |
| E-0103-01 | Hard byte, nesting, field and allocation limits before semantics | Allocation-free scan; 64 KiB body, depth 8, 256 values, 9 fields/object, 8 KiB/string, 16 KiB total strings, 131080-byte decoder capacity; boundary tests, R-NATIVE-01/02 |
| E-0103-02 | Unknown versions/types/fields, duplicate sequence, truncation, trailing data and timeouts fail closed | Protocol/session and actual stdio refusal/deadline/shutdown tests; rejected pre-hello input cannot extend the initial deadline; R-NATIVE-01/02 |
| E-0103-03 | Fuzz and Chromium/Safari differential fixtures without a network listener | Seven production-API fuzz targets, including resumed-hello seeds; pure `native_transport` corpus tests; R-FUZZ-60-M5-ee4216c and R-NATIVE-01; earlier R-FUZZ-60-M5-1f78a3a remains historical |
| E-0104-01 | Approval shows browser, profile class, extension identity, event classes, retained fields and private-context policy | Disclosed-facts test and actual decline/approve/list CLI process test; decline creates no credential, list redacts its secret; R-NATIVE-01, R-PAIRING-01 |
| E-0104-02 | Nonces, expiry, sequence, restart, revocation and updates specified/tested | Pairing/session MAC and expiry tests; fresh process nonce, old-MAC refusal, changed key/permission refusal, CLI revocation; [ADR 0007](../adr/0007-native-reconnect-recovery.md) and uncertain-delivery retry tests; R-NATIVE-01/02, R-PAIRING-01 |
| E-0104-03 | Copied manifests, replaced extensions, replay and stale approvals refuse/re-pair | Copied/unknown identity, digest changes, expiry/revocation, resume forgery, manifest drift preservation, actual caller mismatch and restarted-host replay refusal; R-NATIVE-01, R-PAIRING-01 |

`tests/native_bridge.rs::production_pairing_lifecycle` contains `production_binary_restart_changes_nonce_and_refuses_previous_message_mac`, `production_binary_restart_requires_repairing_after_key_or_permission_change`, `production_binary_restart_observes_revocation_from_a_separate_cli_process`, and `production_pairing_cli_decline_approval_and_list_preserve_the_consent_boundary`. These tests use isolated private homes/stores, real executable processes and private IPC, then reopen the synthetic journal. R-PAIRING-01 records all four passing on the combined merged source. This selection is already included in R-NATIVE-01, so the two selections are not additive.

## Merged-source native and pairing checks

R-NATIVE-01, R-NATIVE-02 and R-PAIRING-01 ran from `2026-10-13T01:40:36.474148Z` to `01:41:00.130974Z` on exact clean merged `ee4216c8fd3ef993538655ac672aa2d37825f6d0`. All selections exited successfully: **98 integration tests plus 4 bridge unit tests, 102 distinct checks**. The four pairing lifecycle process tests were also rerun separately; they are a subset of the 98 integration tests.

The following `/usr/bin/time -l` figures measure each complete Cargo selection, including compilation and test harnesses. They do not measure production-host per-message latency or RSS. No selected test failed or was ignored.

| Receipt | Passed / failed / ignored | Wall s | User CPU s | System CPU s | Maximum RSS bytes | Retained log SHA-256 |
|---|---|---:|---:|---:|---:|---|
| `R-NATIVE-01` | 98 / 0 / 0 | 9.06 | 8.85 | 3.16 | 272,564,224 | `79f13cbd602dfa19d8ea7442f17eb1879021e18a9f6c98d1a6494dd27cc311a1` |
| `R-NATIVE-02` | 4 / 0 / 0 | 13.28 | 15.01 | 1.14 | 947,191,808 | `4aa7328fd90c330ab0dce8601a43bc082361b8a4582ec56eeac179c995483078` |
| `R-PAIRING-01` | 4 / 0 / 0 | 1.15 | 0.19 | 0.17 | 65,257,472 | `03c80a3267c3beb444fe740dca7d8a35cbf5765c6b9560118c1f6da21d845605` |

## Merged-source M5 fuzz campaign

R-FUZZ-60-M5-ee4216c ran on exact clean merged `ee4216c8fd3ef993538655ac672aa2d37825f6d0`, from `2026-10-13T01:40:37.055405Z` to `01:49:55.004155Z`. All seven builds, seed replays and 60-second campaigns passed: **18,025,804 executions**, peak reported RSS **168 MB**, **11 seed replays**, no failure artifacts, unchanged seeds and input manifest. Every retained build/replay/campaign log digest was checked against the original file.

The limits below also apply to the earlier M5 campaign: `-max_total_time=60`, `-timeout=2`, `-rss_limit_mb=256`, `-seed=1`, `-print_final_stats=1`, ASAN `quarantine_size_mb=16`, maximum input 65536 bytes or 131080 for `frame_decoder`, 900-second outer build deadline, 92-second outer campaign deadline and 4194304-byte per-log cap. Wall/child CPU values are measured campaign subprocess seconds, rounded to three decimals; RSS uses libFuzzer's reported MB unit.

| Target | Replays | Executions | Peak RSS MB | Wall s | Child CPU s | Outcome / failure artifacts |
|---|---:|---:|---:|---:|---:|---|
| `frame_decoder` | 2 | 3,118,911 | 91 | 61.570 | 59.546 | passed / 0 |
| `schema_parser` | 3 | 4,298,047 | 168 | 61.496 | 60.882 | passed / 0 |
| `origin_validation` | 1 | 4,349,671 | 90 | 61.503 | 61.038 | passed / 0 |
| `pairing_state` | 1 | 2,950,616 | 102 | 61.502 | 60.907 | passed / 0 |
| `sequence_handling` | 1 | 345,814 | 83 | 61.506 | 60.927 | passed / 0 |
| `policy_conversion` | 1 | 1,430,652 | 142 | 61.548 | 60.605 | passed / 0 |
| `transport_differential` | 2 | 1,532,093 | 107 | 61.599 | 59.330 | passed / 0 |

## Earlier M5 fuzz campaign

R-FUZZ-60-M5-1f78a3a ran on exact clean merged `1f78a3aa92eaee63fdd7012bf69d0b978499ccc6`, from `2026-10-11T02:19:58.595320Z` to `02:27:26.171639Z`. All seven builds, seed replays and campaigns passed: **8,417,188 executions**, peak reported RSS **139 MB**, **11 seed replays**, no failure artifacts, unchanged seeds and input manifest. This is post-merge #433 evidence; it is not reproduction of the later combined native/pairing revision.

This earlier run used the same campaign bounds above. Its measurements and receipt remain scoped to `1f78a3a`.

| Target | Replays | Executions | Peak RSS MB | Wall s | Child CPU s | Outcome / failure artifacts |
|---|---:|---:|---:|---:|---:|---|
| `frame_decoder` | 2 | 1,293,272 | 91 | 61.592 | 43.428 | passed / 0 |
| `schema_parser` | 3 | 973,313 | 137 | 62.669 | 39.007 | passed / 0 |
| `origin_validation` | 1 | 1,946,293 | 89 | 62.464 | 58.100 | passed / 0 |
| `pairing_state` | 1 | 811,276 | 100 | 62.596 | 43.348 | passed / 0 |
| `sequence_handling` | 1 | 200,884 | 83 | 62.403 | 52.433 | passed / 0 |
| `policy_conversion` | 1 | 1,336,711 | 139 | 61.518 | 57.754 | passed / 0 |
| `transport_differential` | 2 | 1,855,439 | 100 | 61.520 | 60.324 | passed / 0 |

## Review and protected-main delivery

R-DELIVERY-01 was observed on 2026-10-13. Each implementation was independently reviewed locally and merged through protected `main`; the retained GitHub metadata and check-run IDs bind the following exact heads to their merge commits. Counts include push and pull-request runs, rather than counting only unique required check names.

| PR | Reviewed head | Merge commit | Retained hosted check runs |
|---|---|---|---|
| [#430](https://github.com/AlisinaDevelo/GHOSTRACE/pull/430) | `566329cca1c1e9f35f6f5330f97e3f6f46d842f6` | `f3fe7c1a7fa9c9f8b6124cdfca38532ecc5cb5ed` | 23, all success |
| [#433](https://github.com/AlisinaDevelo/GHOSTRACE/pull/433) | `56fe461fd0ae86d1f305bd7f06b80089e3031d81` | `1f78a3aa92eaee63fdd7012bf69d0b978499ccc6` | 25, all success |
| [#431](https://github.com/AlisinaDevelo/GHOSTRACE/pull/431) | `e886e997145446b5b94758875d3912da4d847b91` | `b2f86a20bd7ee3b8e02dc98237a2218a1e1dd8f8` | 25, all success |
| [#434](https://github.com/AlisinaDevelo/GHOSTRACE/pull/434) | `a9ca111ced5f1ddd9b96e96b3c5edc7e107da16e` | `ee4216c8fd3ef993538655ac672aa2d37825f6d0` | 25, all success |

The PR #434 candidate and merged `ee4216c` have the same tree, `b8d4268932cd07d46a5e90a4f2cd14e13125d04f`. Protected `main` required strict current-base success for `audit`, `clippy`, `deny`, `rustfmt`, `test (Linux MSRV)`, `test (Linux stable)`, `test (macOS stable)` and `roadmap`, with zero required formal approvals. Independent local review does not claim a hosted formal approval. Hosted checks bind the PR heads; the target-device native and fuzz receipts above separately bind exact merged `ee4216c`.

## Retained artifact IDs

These IDs identify retained original evidence; this page publishes summaries and hashes, not private absolute paths or raw journals. The combined-source campaign has its own observation ID and measurements; the `1f78a3a` run remains separate.

| Artifact ID | Source / scope / state | SHA-256 |
|---|---|---|
| `R-FUZZ-60-M5-1f78a3a/receipt.json` | Clean merged M5 campaign, observed; seven per-target receipts, exact commands, input manifest and build/replay/campaign log hashes retained | `1612b3bb0fd0dc62ed7bb002c7f8775964bf959c172e09923b6728535d5372c4` |
| `R-FUZZ-CI-11689810518.zip` | Hosted Linux push artifact; source `e886e997145446b5b94758875d3912da4d847b91`, `source_tree_dirty: false`, all seven passed, 36,314,465 executions; supporting CI evidence | `77df8c766491fc539cb8fc53708d15eb957a720a9ce4567a8dc7cd87aeca5cdd` |
| `R-FUZZ-CI-11688568573.zip` | Original #433 hosted artifact; all seven passed, `source_tree_dirty: true` from generated Python bytecode; retains this reproducibility limitation, not clean target-device evidence | `043a943f60751d2f3084b318fffe0f34218af76e8792e4d213012a71caf4b564` |
| `R-NATIVE-M5-ee4216c/receipt.json` | Clean combined protected-main source; R-NATIVE-01/02 and R-PAIRING-01 results, exact tool/device identities, commands and measurements | `50be276cbf70dbd58606685bfd086f30684c45930815d9d2f111b20286e6d111` |
| `R-DELIVERY-01/receipt.json` | Independent local review, passing hosted check runs on each exact PR head, protected-main rules and candidate/merged tree equality | `f9a35341dd8295938b45ffb57dce8323ffbaf6fce5c909ce67626683d1e1fc73` |
| `R-FUZZ-60-M5-ee4216c/receipt.json` | Clean combined protected-main M5 campaign; all seven passed with original per-target receipts and build/replay/campaign logs | `c275a5df87a4862dee9c03264ca029f45bac49654aa761ad3dca9f95cf9edd58` |

Each row below names `R-FUZZ-60-M5-ee4216c/<target>/replay.log` and `.../campaign.log` respectively. The receipt also retains each build log digest.

| Target | Replay log SHA-256 | Campaign log SHA-256 |
|---|---|---|
| `frame_decoder` | `7388fc5fca3682639e46843e8fe5632964c47b4951963776d0f14b72d6e56a53` | `683b856abf58166f59588fa0dbb733d5a9f351ad2ee2c26b03a900e63817b665` |
| `schema_parser` | `531a82d43d06d0940d3bd02ed53be8daf1ae1a78dc9db0da3bd7779b53f704cb` | `cdc954af057df1644161895bd22804580d2e88f0ad26648634c806722b1e93b2` |
| `origin_validation` | `2ac6cf5a493a91871d367d9eed69398d6cb59d1ab7557866bab8a8a21b19c173` | `88a1c1f56920615c6f6b5aa2aa402c6da9f203d703ac0196cc4f8031a119abe7` |
| `pairing_state` | `5be089a050cf6a198e0c1dd67c772e792b51c5679d0a0d7555ed40951b0b9104` | `709199c8eacd6e0bbd10d85293f5023809c4da3a021e57d4ef5dc0808b2c663a` |
| `sequence_handling` | `e56eb5b560e7cd0d0feca1b140b11658c7dc0670f00b48890dfe88b380dec5d4` | `415fc352f169b0fd0bf943cd2dc8c0a20cb885f61df2292f4e5d43ae6b91ba12` |
| `policy_conversion` | `ba218f2d285ae7a417b290a55ba351df710ee0d4b3f30b3a1f8c25e928466846` | `cc5e679a4b5a3ace893dc84b33de68e888ca478cac09ccde86486766c316857a` |
| `transport_differential` | `fd8a5c79e78d14af934f45967a464441b4b8d14e1fa7fdfdadfbf89ad43f5331` | `062ec4927e43fbb02bbe4de3623e00d1cbaa566f4e1bf89836774f04cd28a237` |

The historical rows below name `R-FUZZ-60-M5-1f78a3a/<target>/replay.log` and `.../campaign.log` respectively. Per-target receipts also retain build outcomes and log hashes.

| Target | Replay log SHA-256 | Campaign log SHA-256 |
|---|---|---|
| `frame_decoder` | `6aaff7a6be7b7537169a79cb40420cbe0090414f8587d02a30500c8bd9269009` | `1ad016dc5881b762bed504084f3669705aae7087b88debf046552ed6596c3fc4` |
| `schema_parser` | `a1d517754fbd26c254f6c134186c7cec26d8f5b9b51a97e5fcb8b566d9d3243b` | `25ae2ee2e3ac7c9471ad87cf1fed1844787b4f64a4606ba9d874316a5e05da15` |
| `origin_validation` | `3689cbd11e3b42f2f1ec6edf407eb15111223b3508228168eac7351b0ad28d55` | `9e6b29c9776c4606ac9b09fdd5cc1967119bbad2db27f992dcfc37cdb1f71fe1` |
| `pairing_state` | `1a2d1b1fc4a51e5e774758da3efda7b32cb65754de70f483b79347df8659ac6f` | `a2acb60193581e53877b6fd0c6ad8ca4bb13e202155a53cb8855ce0feeee5844` |
| `sequence_handling` | `b088a7a23a7d76f23b07eaf39be95a388b4d16d0c139ce759aa1c31d30eac00d` | `d2c423a6647900f66a7cf6ea21133e89864b93f6288b5180479d2a2719c31f73` |
| `policy_conversion` | `005875b4387d5e6d4b3145b018a11151c4c2880a8700ffc468756e816d6f1008` | `60b6c1ac491da7825270b7bc60553b55a1df0395666d9d51921c1369c3c836e0` |
| `transport_differential` | `29296ebf7b4db3757da7f82ade708e0cb1fc4219fb956d119d17e9adfabc2273` | `9e9d254086eb4fd45e7d85d17dd0e67d58fe49e5faee3921ff4d771598f8c40f` |

Historical M1/source `51657a2` and candidate one-second runs remain historical observations in their existing receipts; neither is substituted for the M5 60-second campaign.

## Reproduction commands

Run from a clean checkout of the recorded source and retain exit codes, UTC times, exact tool identities and sanitized log digests. These focused selections do not replace the repository-wide or hosted delivery checks in CONTRIBUTING.md. The pairing selection is also included in the full native-bridge selection; its count is not additive.

```sh
cargo +1.88.0 test --locked --all-features \
  --test native_messaging --test native_transport --test browser_pairing \
  --test native_host_manifest --test native_bridge \
  --test local_service --test local_service_browser_ingest -- --test-threads=1
cargo +1.88.0 test --locked --all-features --lib native_bridge::tests -- --test-threads=1
cargo +1.88.0 test --locked --all-features --test native_bridge \
  production_pairing_lifecycle -- --test-threads=1

cargo +nightly-2026-10-03 fetch --locked --manifest-path fuzz/Cargo.toml
PYTHONDONTWRITEBYTECODE=1 RUSTUP_TOOLCHAIN=nightly-2026-10-03 \
  FUZZ_TARGET_TRIPLE=aarch64-apple-darwin CARGO_BUILD_JOBS=1 \
  FUZZ_SECONDS=60 FUZZ_TIMEOUT_SECS=2 FUZZ_RSS_MB=256 FUZZ_MAX_LEN=65536 \
  FUZZ_BUILD_TIMEOUT_SECS=900 FUZZ_MAX_LOG_BYTES=4194304 \
  FUZZ_BUILD_DIR=fuzz/target/bounded FUZZ_OUTPUT_DIR=fuzz/artifacts/final-MERGED_SHA \
  ASAN_OPTIONS=quarantine_size_mb=16 sh fuzz/run-bounded.sh
```

## Scope and remaining gates

| Evidence scope | Established by named checks | Separate or pending gate |
|---|---|---|
| Happy path | Actual host executable, independently authenticated private Unix IPC, durable origin-only synthetic journal; explicit CLI consent facts | Installed browser collector, shipped long-running service owner, signed distribution |
| Negative/privacy | Identity/MAC/receipt refusals, path-retention/private-context refusal, sentinel exclusion, secret-redacted list and encrypted store | Live browser private-mode permissions; full sidecar/export/support privacy matrix; stolen pairing/store keys |
| Failure/recovery | Frame/schema/deadline/shutdown refusals; admission fence and writer uncertainty; process restart/update-digest/revocation; durable retry and honest gaps | Actual installed-browser restart/update behavior |
| Resource | Protocol/allocation/rate/lock/output bounds; observed synthetic fuzz wall/CPU/RSS and wrapper/log limits | Accepted/rejected production host CPU/RSS, message/queue/disk/energy budgets and secret-diagnostic fault preservation |

Pairing uses declared credential scope, not OS-attested browser identity. Manifest registration permits host launch; user consent is separate. The pairing store/key is separate from the journal key. Approval is the CLI. The controlled test service owner does not prove a shipped service lifecycle. Recovery is stop-and-wait with one outstanding input, latest processed receipt, fresh host nonce and a new MAC; arbitrary pipelining and stale-receipt reconstruction are unsupported. Old strict reply parsers need migration for the optional receipt field; no shipped client compatibility is claimed. Fuzz targets do not persist events or stress the writer queue, and Safari-shaped normalization does not claim Safari support.

All ten criteria above have merged-source target-device proof and retained delivery binding. Dependencies `[0008, 0010, 0029, 0102, 0103, 0104]` are verified done, and task 0030 meets its capstone criteria within the declared scope. Task 0109/#113 stays **in-progress and open**: the continuous CI campaign and retained artifacts are implemented, but reviewed minimized-regression preservation, full accepted/rejected production resource coverage, and secret-bearing diagnostic/oracle gates remain. The runner hashes failure inputs and does not automatically minimize them.

Shipped Chromium collection (0031/0105), the browser privacy matrix (0033/0108), Safari go/no-go (0034/0110), service lifecycle (0035), service resource/fuzz coverage (0112), and release/signing gates remain separate.
