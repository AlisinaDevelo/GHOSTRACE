# Task 0109 candidate evidence: hostile native-boundary fuzzing

Status: historical campaign and later merged-runner observations are recorded
below. Combined merged-source native/pairing reproduction is retained in the
[current completion evidence](0030-native-host-completion.md). Task 0109 stays
in-progress and open; a build or corpus alone is not a campaign.

`fuzz/Cargo.toml` defines seven corpus-guided libFuzzer targets: frame decoder,
schema parser, origin validation, pairing state, sequence handling, policy
conversion, and pure Chromium/Safari transport differential normalization.
Each target bounds input before calling production APIs. Seeds are synthetic;
the targets do not open listeners, launch extensions, touch persistence, or
emit raw inputs in errors. `fuzz/run-bounded.sh` serializes targets and caps
time, per-input timeout, RSS, and input length.

## Merged runner and current observations

[PR #433](https://github.com/AlisinaDevelo/GHOSTRACE/pull/433) merged at
`1f78a3aa92eaee63fdd7012bf69d0b978499ccc6`, adding pinned tools,
seed/regression replay, bounded subprocesses/logs, receipts and continuous CI
artifact retention. A clean-source M5 run on 2026-10-11 completed all seven
60-second targets: 8,417,188 executions, peak reported RSS 139 MB, all passed,
no failure artifacts, unchanged inputs. Artifact
`R-FUZZ-60-M5-1f78a3a/receipt.json` has SHA-256
`1612b3bb0fd0dc62ed7bb002c7f8775964bf959c172e09923b6728535d5372c4`.
Its tool/device/per-target receipts are in the current completion evidence;
these measurements remain bound to `1f78a3a`, not a later combined revision.

[PR #431](https://github.com/AlisinaDevelo/GHOSTRACE/pull/431), merged
`b2f86a20bd7ee3b8e02dc98237a2218a1e1dd8f8`, aligned action pins and set
`PYTHONDONTWRITEBYTECODE`. Hosted Linux artifact `R-FUZZ-CI-11689810518.zip`
records clean source `e886e997145446b5b94758875d3912da4d847b91`, seven passing
targets and 36,314,465 executions; ZIP SHA-256 is
`77df8c766491fc539cb8fc53708d15eb957a720a9ce4567a8dc7cd87aeca5cdd`.
Original hosted artifact 11688568573 also passed seven targets but recorded
`source_tree_dirty: true` from generated Python bytecode. Retain that limitation;
it is not clean target-device evidence.

Native-host ingress is integrated; reconnect recovery merged in
[PR #430](https://github.com/AlisinaDevelo/GHOSTRACE/pull/430),
`f3fe7c1a7fa9c9f8b6124cdfca38532ecc5cb5ed`. Fuzz campaigns exercise production
APIs without running the actual native process or writer queue. Reviewed
minimized regressions, accepted/rejected production host/service resource
coverage and secret-bearing diagnostic/fault-oracle preservation remain task
0109 gates. The runner retains hashed failures; it does not automatically
minimize them.

## Historical candidate and interrupted attempt

The original candidate had no campaign evidence. The 2026-10-04 run below
supersedes that limitation for revision `51657a2` only. The first local attempt
of the 2026-10-11 CI candidate was interrupted during its first target and is
incomplete. Candidate one-second runs are short observations, not 60-second
qualification. None of these source-scoped records is relabelled as the clean
merged M5 run.

The transport differential target is intentionally not a live Safari test.
Safari app-extension packaging, permissions, runtime messaging, and target
device parity remain task 0034/0110 gates.

## Historical M1 bounded campaign receipt

Run on 2026-10-04 on the reference M1 against branch head `51657a2`, with
`RUSTUP_TOOLCHAIN=nightly FUZZ_TARGET_TRIPLE=aarch64-apple-darwin sh fuzz/run-bounded.sh`
(60 s per target, 2 s per-input timeout, 256 MB RSS cap, 64 KiB maximum input,
128 KiB for `frame_decoder`), `rustc 1.101.0-nightly (0abfedbc7 2026-10-02)`,
cargo-fuzz 0.13.2. The original receipt identifies historical logs as artifact `fuzz-51657a2`; these logs were not rerun or newly verified for this ledger.

| Target | Executions | Peak RSS (MB) | Failures | Log SHA-256 |
|---|---:|---:|---:|---|
| `frame_decoder` | 2,189,498 | 52 | 0 | `20303b0e9575ee8d0973b88f7c2af6d8623d3fef9547cbbdf7358c8fdf9930de` |
| `schema_parser` | 2,180,045 | 73 | 0 | `7c1bdcd957bbe0a0d5f5aa8618102353b4b292ce4613272c7d1ae3f8cda35466` |
| `origin_validation` | 2,871,221 | 87 | 0 | `205af620555e2e271f98425e5edda0b1d19b9944cdea546312e3574966fb91c3` |
| `pairing_state` | 1,966,775 | 90 | 0 | `615c564fe5a61b7a1464aa940ed022f12aa09360a6f5158578bc3f28a1b1b2c8` |
| `sequence_handling` | 182,577 | 80 | 0 | `ce6847d6f72be0c211164ce735dfeea09fc05cdea54f605303a31ad31003d26f` |
| `policy_conversion` | 948,489 | 71 | 0 | `adc6b9d1ba541f8e16e93421ece77cb8331a6e18acf553648807c369b35ae75c` |
| `transport_differential` | 1,345,603 | 77 | 0 | `bf51ed045032d184c28a40713d4681f592e1145ad3bdb592e63eb5668aa1776e` |

The corrected-ASAN M1 run recorded 11,684,208 executions in total, no crash,
panic, timeout or out-of-memory. These are historical M1/source `51657a2`
measurements, not current M5 measurements.

An earlier M1 run stopped `policy_conversion` and `transport_differential` with
out-of-memory reports at 424 MB and 338 MB. The saved input was empty and replays
in 1 ms; the growth came from AddressSanitizer's default 256 MB quarantine of freed
memory, which by itself fills a 256 MB cap on allocation-heavy targets. With a
16 MB quarantine the same target ran 878,714 times with peak RSS flat at 89 MB.
`run-bounded.sh` now defaults `ASAN_OPTIONS=quarantine_size_mb=16`, keeping
use-after-free detection while the cap measures the code under test.
