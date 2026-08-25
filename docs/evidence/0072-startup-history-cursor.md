# Task 0072 evidence: startup history and cursor gates

Status: implementation and target-device verification complete. The
implementation is protected on `main` at `906dc362e13ec96b10be607e8961c066bbd8d9a0`
(PR [#256](https://github.com/AlisinaDevelo/GHOSTRACE/pull/256)).
The post-merge hosted workflows for that exact protected commit all passed:
[CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32846970296),
[Rust advisories](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32846970256),
[offline fixture](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32846970342),
and [Cargo policy](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32846970298).

## Contract

FSEvents startup has two explicit modes. `SinceNow` is a live-start decision;
an ordered nonzero event ID is a replay decision. A replaying collector exposes
`Replaying` and may become `Live` only when the native `HistoryDone` boundary is
consumed. The boundary is control evidence, not a filesystem observation.

Zero, stale, future, wrapped, and corrupted positions are refused rather than
silently downgraded to `SinceNow`. A bounded timeout, partial-history source
status, or explicit stop before `HistoryDone` emits a path-free history gap with
`reinitialize_stream`, sets `HistoryUnavailable`, and keeps
`recovery_required` asserted.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| HistoryDone is a state transition and never a user event | `tests/selected_root_collector.rs::history_done_transitions_replaying_to_live_without_user_event` drives native macOS FSEvents replay, observes `Replaying` → `Live`, and asserts no `Gap` or `FilesystemChanged` record for the sentinel |
| Cursor modes and invalid positions are explicit | `tests/fsevents_startup.rs::startup_cursor_decisions_make_since_now_and_replay_explicit`, `startup_cursor_refuses_zero_stale_future_wrapped_and_corrupt_inputs`, and the strict `schemas/fsevents-startup-v1.json` contract |
| Incomplete replay cannot claim live coverage | `tests/selected_root_collector.rs::incomplete_history_emits_a_gap_and_never_reports_live` interrupts replay before a flush, observes `fsevents_history_incomplete`, `HistoryUnavailable`, and `recovery_required` |
| Timeout and partial-history gates are implemented and exercised | `tests/selected_root_collector.rs::history_timeout_emits_a_gap_before_native_flush_can_claim_live` sleeps beyond a 1 ms budget before a native pump and observes `fsevents_history_timeout`; `FseventsCollector::is_partial_history_failure` emits the bounded `fsevents_history_partial` variant with `ReinitializeStream`; the source adapter remains gated until reconciliation |
| Public documentation and privacy boundaries are aligned | `docs/ARCHITECTURE.md`, `docs/EVENT_MODEL.md`, and `docs/PRIVACY.md` describe readiness, gap reason codes, and the no-path/no-content contract |

## Target-device verification

The exact protected implementation commit was checked on macOS 26.6.2
(25G83), Apple M1 arm64, Rust 1.88.0, Cargo 1.88.0, and Python 3.9.6. Receipt
hashes are SHA-256 of complete stdout/stderr logs.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo +1.88.0 fmt --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed; 31 unit tests and all integration targets; one explicit Keychain authorization test ignored | `3fdf6e1e3783245674177ab47fc97c5fa3b0876f5e10d7591d62456148e68c78` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `bfc33773865ecee468d754a05bbdad2fc3de8be2a9eb4b5c9d80e230840b4515` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `19ab943dd1741a8e723cd890de64253e60c903cca987e2efb24e87acaa32e132` |
| `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --all-features --no-deps` | passed | `155bbd45d76da815816ace46f1ee30d87250465a8d8df48796f24ebcdec409ab` |
| `./scripts/offline-network-test.sh` | passed; denial canary enforced and full offline suite green | `2f6b656600411f369f22fcafcdd63d41042d11d2de7cad14e9a7197af1ddcff9` |
| `./scripts/reproducibility-test.sh` | passed; 40 Python tests and deterministic fixture/replay/export lanes green | `290ae1c6c60e65e72f4464e97c7aeeb8415fbb89a8a6b00e75f1f0c495fc688e` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; 40/40 | `b929d895adf602e58ff58c6215c6ea2db9cec82bf23b2ec47a22e5e6cd00072b` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 488 dependency edges, 0 blocked | `b7fac9423fe493f5ef31e6c1524f35973c0d4190b83781322beb3b383e6ec212` |
| `cargo +1.88.0 check --locked --target x86_64-unknown-linux-gnu --all-targets --all-features` | cross-target attempt was blocked locally because no `x86_64-linux-gnu-gcc` is installed; hosted Linux stable/MSRV checks passed after the portability fix | `520378b10eff0ae5040d369d50743f683eb6839cc919a6113aafede3f3217901` |

The locked-session Keychain lifecycle test still requires explicit device
authorization and remains reported as ignored/unavailable rather than being
substituted by hosted CI. No path, display name, account data, credential, or
capture key is retained in this evidence.
