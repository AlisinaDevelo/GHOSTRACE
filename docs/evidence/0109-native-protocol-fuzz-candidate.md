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
