# Task 0024 evidence: explicit shell-wrapper metadata capture

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#344](https://github.com/AlisinaDevelo/GHOSTRACE/pull/344)
was squash-merged to protected `main` at
`905c11e6895a63c6a5b38a2973e64686b9321b4b`.

The deliverable is `ShellWrapper` in `src/shell_wrapper.rs`, a consent-gated
library adapter. There is no CLI entry point yet (task 0161,
[#346](https://github.com/AlisinaDevelo/GHOSTRACE/issues/346)) and no ambient shell
capture.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0024-01 | Capture occurs only through the explicit run wrapper. | `ShellWrapper::new` requires a confirmed consent preview for a policy enabling the `shell` source and builds a live origin only inside the adapter. `run`/`run_in` execute exactly one caller-supplied program; revoked consent refuses before spawning (`revoked_consent_refuses_before_spawning`), and a policy without the shell source or with an unselected workspace is refused (`policy_without_the_shell_source_is_refused`, `workspace_must_be_selected_by_policy`). The CLI `capture` command remains disabled. |
| E-0024-02 | Executable, timing, exit status, and sanitized working directory are captured. | `shell_started` carries the normalized executable basename token and a working-directory class plus device/inode-anchored digest; `shell_finished` carries status, exit code, duration, and signal, parented to the start. `completed_run_records_started_and_finished_metadata_only`, `failure_and_signal_outcomes_propagate_unchanged`, `working_directory_is_classified_and_digested_without_path_text`, `executable_identity_is_a_normalized_basename_or_unclassified`, and `wrapper_events_validate_against_the_published_event_schema` pass. Exec failure is a `shell_exec_failed` gap with no fabricated end (`exec_failure_is_a_gap_without_a_fabricated_end_or_status`). |
| E-0024-03 | Arguments, environment, standard input, and output are not captured. | The child inherits the terminal and environment, but no argument, environment, or stream byte is read, stored, or hashed; the payloads have no field for them. Sentinel checks over the serialized journal find no argument, command text, environment name or value, or raw path (`environment_and_standard_streams_never_reach_the_journal` and the success/exec-failure tests). |

## Delivery

- Issue: [#28](https://github.com/AlisinaDevelo/GHOSTRACE/issues/28)
- Implementation PR: [#344](https://github.com/AlisinaDevelo/GHOSTRACE/pull/344)
- Implementation commit before squash: `196d691e7c7eadd52473113cbab8ba9946c2721e`
- Protected-main merge: `905c11e6895a63c6a5b38a2973e64686b9321b4b`
- Verification date: 2026-09-27 UTC

## Device and toolchain

```text
Darwin 25.6.0 / macOS 26.6.2 / MacBookPro17,1 / Apple M1 / arm64 / 8 logical CPUs
rustc 1.88.0 (6b00bc388 2025-06-23), host aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 905c11e6895a63c6a5b38a2973e64686b9321b4b
```

## Merged-main device verification

Every command ran from protected `main` at
`905c11e6895a63c6a5b38a2973e64686b9321b4b`, which contains this task's merge.
Hosted checks are corroboration; the retained device logs are the acceptance
evidence.

- `bash scripts/reproducibility-test.sh` exited `0`: pinned inputs, rustfmt,
  schema, Parquet profile, shell metadata/lifecycle/leakage, Git identity, Git
  snapshot privacy, deterministic demo/journal/export/retention/integrity/
  authenticated-state/recovery flows, capture refusal, the Python evidence, and
  the Rust evidence all passed.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited
  `0`: 274 passed, 0 failed, 3 ignored.
- `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps` exited `0`.
- `cargo +1.88.0 build --release --locked` exited `0`.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 51 tests.

Not run in this verification: the sandboxed `scripts/offline-network-test.sh`
lane and the native filesystem benchmark. The benchmark's standing resource
no-go on this device is tracked separately in task 0163
([#348](https://github.com/AlisinaDevelo/GHOSTRACE/issues/348)); it is not
claimed as a pass here.

Focused optimized runs, each `cargo +1.88.0 test --release --locked --test <suite>
-- --nocapture`, exited `0`: `shell_run_wrapper` 10/10, `shell_metadata` 4/4,
`shell_wrapper_lifecycle` 7/7, `shell_secret_leakage` 6/6.

## Retained artifacts

Logs are retained on the verification device outside the repository.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-905c11e/device-info.txt` | exact device/toolchain capture | `adb8595f0d125f6f101683a3611f9283f2c608861380a91203c3f33d3aeced09` | 230 |
| `ghostrace-evidence/merged-905c11e/repro.log` | reproducibility lane, exit 0 | `d6ac200dbeb799def517ed268ab0889130fb7318fb389bc48a975cb1ee2c1515` | 41622 |
| `ghostrace-evidence/merged-905c11e/clippy.log` | Clippy with warnings denied, exit 0 | `ddcb06ce53600b5701e9bb1c7de36d92136d672683aa24d86c6993e714b4e4ec` | 114 |
| `ghostrace-evidence/merged-905c11e/all-targets.log` | all targets except native benchmark, 274 passed | `0e3d39a527e6d18659fc2343e6dbf827f5c5e94687786ab82adc3b226db9528e` | 34606 |
| `ghostrace-evidence/merged-905c11e/rustdoc.log` | rustdoc with warnings denied, exit 0 | `2409b54ac218a6e0c805348e3f53dc92ce786a1c947fb00d5816c5d24c049e7d` | 602 |
| `ghostrace-evidence/merged-905c11e/release-build.log` | optimized release build, exit 0 | `09188ec28550ccfa239af0cd174aa6597d532e3d6b61ffac4c2449c1cdd7943d` | 3438 |
| `ghostrace-evidence/merged-905c11e/release-shell_run_wrapper.log` | optimized wrapper suite, 10/10 | `1eeea6bcaa2b2db95510cc106a13f68789a82219dbac7acc3074b655248516d4` | 1106 |
| `ghostrace-evidence/merged-905c11e/release-shell_metadata.log` | optimized metadata suite, 4/4 | `0023889214316224b2ace5b19fb021a89104ee43beeaa79000e61fdcc7c21e72` | 694 |
| `ghostrace-evidence/merged-905c11e/release-shell_wrapper_lifecycle.log` | optimized lifecycle suite, 7/7 | `3f7779a13d7ca88a7bcb3ee0ddd7b13465c9dd1eebb477a02b45384223941ce0` | 943 |
| `ghostrace-evidence/merged-905c11e/release-shell_secret_leakage.log` | optimized leakage suite, 6/6 | `3ca97d98e4578f98416c4104bae0d454959111ccfda4b7b3917041d01d504a4f` | 826 |
| `ghostrace-evidence/merged-905c11e/python.log` | Python suite, 51/51 | `1ae2f4741ce4a6f4dac9b141f2500cfae4d87e6edee92439286e5f49716c50d4` | 150 |

## Privacy, failure, and scope boundaries

- Tests run only fixed synthetic commands (`/bin/sh -c` scripts, `/usr/bin/true`,
  `/usr/bin/touch`) in temporary directories; no user command or path is recorded.
- `executable_id`, `working_directory`, and `signal` are optional v1 payload fields
  omitted when absent, so pre-existing envelopes and fixtures are byte-identical.
  A reader built before these fields would reject envelopes that carry them; none
  has been released.
- The working-directory digest is anchored to the scope directory's device/inode,
  and outside any scope to the directory's own device/inode, so it cannot be
  matched against a dictionary of path strings. It is not an irreversibility
  claim against an attacker with access to the same filesystem.
- A wrapper killed before its child exits leaves an unmatched start; forwarding of
  SIGTERM/SIGHUP and recovery of such runs are task 0162
  ([#347](https://github.com/AlisinaDevelo/GHOSTRACE/issues/347)).
