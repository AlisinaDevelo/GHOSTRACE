# Task 0109 candidate evidence: hostile native-boundary fuzzing

Status: candidate package only. A fuzz-crate build or checked-in corpus is not
a fuzzing campaign and this document does not support issue closure.

`fuzz/Cargo.toml` defines seven corpus-guided libFuzzer targets: frame decoder,
schema parser, origin validation, pairing state, sequence handling, policy
conversion, and pure Chromium/Safari transport differential normalization.
Each target bounds input before calling production APIs. Seeds are synthetic;
the targets do not open listeners, launch extensions, touch persistence, or
emit raw inputs in errors. `fuzz/run-bounded.sh` serializes targets and caps
time, per-input timeout, RSS, and input length.

No campaign was run for this candidate branch because the shared Rust build
resource was reserved for the parent integration health check. There are no
minimized regressions, hang/crash receipts, execution counts, peak-RSS
measurements, or diagnostic-output checks to report yet. Those are required
before task 0109 can be marked complete.

The transport differential target is intentionally not a live Safari test.
Safari app-extension packaging, permissions, runtime messaging, and target
device parity remain task 0034/0110 gates.

## Bounded campaign receipt

Run on 2026-10-04 on the reference M1 against branch head `51657a2`, with
`RUSTUP_TOOLCHAIN=nightly FUZZ_TARGET_TRIPLE=aarch64-apple-darwin sh fuzz/run-bounded.sh`
(60 s per target, 2 s per-input timeout, 256 MB RSS cap, 64 KiB maximum input,
128 KiB for `frame_decoder`), `rustc 1.101.0-nightly (0abfedbc7 2026-10-02)`, cargo-fuzz 0.13.2. Logs are kept in
`~/Developer/dev/ghostrace-evidence/fuzz-51657a2`.

| Target | Executions | Peak RSS (MB) | Failures | Log SHA-256 |
|---|---:|---:|---:|---|
| `frame_decoder` | 2,189,498 | 52 | 0 | `20303b0e9575ee8d0973b88f7c2af6d8623d3fef9547cbbdf7358c8fdf9930de` |
| `schema_parser` | 2,180,045 | 73 | 0 | `7c1bdcd957bbe0a0d5f5aa8618102353b4b292ce4613272c7d1ae3f8cda35466` |
| `origin_validation` | 2,871,221 | 87 | 0 | `205af620555e2e271f98425e5edda0b1d19b9944cdea546312e3574966fb91c3` |
| `pairing_state` | 1,966,775 | 90 | 0 | `615c564fe5a61b7a1464aa940ed022f12aa09360a6f5158578bc3f28a1b1b2c8` |
| `sequence_handling` | 182,577 | 80 | 0 | `ce6847d6f72be0c211164ce735dfeea09fc05cdea54f605303a31ad31003d26f` |
| `policy_conversion` | 948,489 | 71 | 0 | `adc6b9d1ba541f8e16e93421ece77cb8331a6e18acf553648807c369b35ae75c` |
| `transport_differential` | 1,345,603 | 77 | 0 | `bf51ed045032d184c28a40713d4681f592e1145ad3bdb592e63eb5668aa1776e` |

11,684,208 executions in total, no crash, panic, timeout, or out-of-memory.

A first run stopped `policy_conversion` and `transport_differential` with
out-of-memory reports at 424 MB and 338 MB. The saved input was empty and replays
in 1 ms; the growth came from AddressSanitizer's default 256 MB quarantine of freed
memory, which by itself fills a 256 MB cap on allocation-heavy targets. With a
16 MB quarantine the same target ran 878,714 times with peak RSS flat at 89 MB.
`run-bounded.sh` now defaults `ASAN_OPTIONS=quarantine_size_mb=16`, keeping
use-after-free detection while the cap measures the code under test.
