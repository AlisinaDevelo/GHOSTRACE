# Task 0072 evidence: startup history and cursor gates

Status: implementation and protected-main verification complete. The core
implementation is in PR [#256](https://github.com/AlisinaDevelo/GHOSTRACE/pull/256)
at `906dc362e13ec96b10be607e8961c066bbd8d9a0`; the timeout test and evidence
receipt are protected on `main` at `45c755552046d520472f52a44bd3045b4e39d900`
in PR [#257](https://github.com/AlisinaDevelo/GHOSTRACE/pull/257).
The post-merge hosted workflows for the final protected commit all passed:
[CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32847727645),
[Rust advisories](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32847727708),
[offline fixture](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32847727658),
and [Cargo policy](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32847727702).

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

The exact final protected commit `45c755552046d520472f52a44bd3045b4e39d900`
was checked on macOS 26.6.2 (25G83), Apple M1 arm64, Rust 1.88.0, Cargo
1.88.0, and Python 3.9.6. Receipt hashes are SHA-256 of complete
stdout/stderr logs.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo +1.88.0 fmt --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed; 31 unit tests and all integration targets; one explicit Keychain authorization test ignored | `641927a5e9cb63e7185d0119a3537992c4d841702cd359d9271dc04c7c6c0786` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `beed191cf5cd39a57477a6d66e9acb53f3b4efe7566a30e0ada805357a0fe6ff` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `f18e8aa44b81e7d9d1663536017be739eb80ede7041400fe06bf4e1c755d3d59` |
| `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --all-features --no-deps` | passed | `e77bb658b0e39ad801c3692ea0e91894abdad9facb43af5d714e434aa5f920f8` |
| `./scripts/offline-network-test.sh` | passed; denial canary enforced and full offline suite green | `0aaab29a4d63c9c81cb6b962da0e17420c4caacba725345bab6426d158ac190c` |
| `./scripts/reproducibility-test.sh` | passed; 40 Python tests and deterministic fixture/replay/export lanes green | `ad87103943756e006d5dbb4800224428f1a29ac01eb7b3345f09b51a07d8b5ba` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; 40/40 | `9326e11b30c6f829501ae04dc13852e95f67017c0ee6e05c56a53a0142a8fecd` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 488 dependency edges, 0 blocked | `b7fac9423fe493f5ef31e6c1524f35973c0d4190b83781322beb3b383e6ec212` |
| `cargo +1.88.0 check --locked --target x86_64-unknown-linux-gnu --all-targets --all-features` | cross-target attempt was blocked locally because no `x86_64-linux-gnu-gcc` is installed; hosted Linux stable/MSRV checks passed | `09fac9a697a5ecdbd891f4e8895e57969238676941a5679e9e79757b8d3ad311` |

The locked-session Keychain lifecycle test still requires explicit device
authorization and remains reported as ignored/unavailable rather than being
substituted by hosted CI. No path, display name, account data, credential, or
capture key is retained in this evidence.
