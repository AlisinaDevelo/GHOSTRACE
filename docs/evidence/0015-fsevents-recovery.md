# Task 0015 evidence: persisted FSEvents cursor recovery

Status: implementation and protected-main verification complete. PR [#259](https://github.com/AlisinaDevelo/GHOSTRACE/pull/259)
was reviewed and merged to protected `main` at
`9675be84d9abf1c100db266c9bb5523d8ef487f7`.

The merge-triggered hosted workflows for that exact protected commit all
passed: [CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32852040585),
[Rust advisories](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32852040480),
[Cargo policy](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32852040601),
and [offline fixture](https://github.com/AlisinaDevelo/GHOSTRACE/actions/runs/32852040514).

## Contract

An FSEvents restart must use the last cursor whose event was durably committed.
The requested replay boundary remains part of the journal identity; deriving a
native event ID for a restart does not create a new configuration boundary.
When the prior boundary is unavailable, mismatched, invalidated, wrapped, or
malformed, startup emits a path-free recovery gap and refuses to claim live
coverage. The gap and cursor invalidation are committed together. A normal
`cursor_jump` remains an explicit global-ID gap without invalidating the stream,
because jumps outside the selected scope are expected in FSEvents.

## Acceptance mapping

| Criterion | Evidence |
| --- | --- |
| Each cursor and its events commit atomically | `tests/replay_boundary.rs::recovery_gap_invalidates_cursor_atomically_and_survives_reopen` ingests an event and recovery gap, reopens the file journal, and verifies the invalidated cursor and exactly two durable events; existing writer fault tests cover rollback and retry boundaries. |
| Restart resumes from the committed cursor | `tests/selected_root_collector.rs::restart_resumes_from_committed_cursor_and_persists_invalidated_gap` captures a controlled file lifecycle, closes and reopens the journal, and asserts `StartupCursorDecision::Replay` uses the committed event ID. |
| Invalid, wrapped, or dropped history emits a gap | The same selected-root test invalidates the persisted cursor and observes `fsevents_cursor_invalidated` with `reinitialize_stream`; `tests/fsevents_startup.rs` refuses zero, stale, future, wrapped, and corrupt cursors; `tests/fsevents_loss_gaps.rs` covers wrapped/dropped source status and distinct reason codes. |
| The journal never claims completeness across an uncovered interval | Startup recovery gaps have no cursor bounds, require reconciliation, set `HistoryUnavailable`, and make `start()` fail closed; existing timeout, partial-history, root-replacement, and replay-boundary tests verify no silent live transition. |

## Target-device verification

The exact protected commit above was checked from a fresh worktree on macOS
26.6.2 (25G83), MacBookPro17,1 / Apple M1 arm64, Rust 1.88.0, Cargo 1.88.0,
and Python 3.9.6. Receipt hashes below are SHA-256 of complete stdout/stderr
logs from each command.

| Lane | Result | Receipt |
| --- | --- | --- |
| `cargo +1.88.0 fmt --check` | passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` | passed | `81571bc4eed14f84010378974a8783e2ad8f6c310cd4a53d4d1f165391e43cb8` |
| `cargo +1.88.0 test --locked --all-targets --all-features` | passed; 31 unit tests and all integration targets; one explicitly authorized Keychain lifecycle test ignored | `1c4f3cc0ad216a8737f131dc52871e1bf2b17436bc2a222b091108f045cea4bc` |
| `cargo +1.88.0 test --locked --release --all-targets --all-features` | passed; one explicitly authorized Keychain lifecycle test ignored | `834a078233fcc0bb6d68e6fd70d3389eeadd48c1561d0f96f711f368b7df35ff` |
| `cargo +1.88.0 build --locked --all-features --release` | passed | `0624ef8022f86bd0b0734ac4366ac1c242f5316ae09f782c1528105f158050ab` |
| `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked --no-deps --all-features` | passed | `cbe249a67e2c0c288041478b6af2de490a80f1a86501d96fc7c7ab4f76557fcc` |
| `./scripts/offline-network-test.sh` | passed; denial canary enforced and full offline suite green | `d9d57da5d9c6234a3e955d89b6501366ce0d66acd59f4f85342020ac28dd442b` |
| `./scripts/reproducibility-test.sh` | passed; deterministic fixture/replay/export lanes green | `0fdd413332855760bfe9b75d7b2bb2a26ade53f96672428ff3319fc23aa4444f` |
| `python3 -m unittest discover -s tests -p 'test_*.py'` | passed; 40/40 | `218c834d86ac83e11605aa5264617ef95d725a93b37570c69c921542ff55ac4e` |
| `python3 scripts/roadmap.py check` | passed; 160 tasks, 488 dependency edges, 0 blocked | `b7fac9423fe493f5ef31e6c1524f35973c0d4190b83781322beb3b383e6ec212` |
| `python3 scripts/roadmap.py index` | passed | `e2a526c44ecb1be2fdfc5abdf20cd7f542cfe46682c267f03d740926d74909c7` |
| selected-root collector debug test | passed; 8/8 | `f48e525a27d8555b22b3732305b16c19df0677adfe344b5162de762114b68636` |
| selected-root collector release test, run 1 | passed; 8/8 | `73277fcc02db3e9d21ca4289039d61c5a3c849aa30366b0bec50efb9b265fd18` |
| selected-root collector release test, run 2 | passed; 8/8 | `91c26af793d82d7ad119ade1355b2b65781f35444ea644569d940e7071bb16bf` |
| selected-root collector release test, run 3 | passed; 8/8 | `8c5620dab214d4e33c338a8483b345d1cdc9a5756d6c6c3f736a3536d393fe35` |
| Linux cross-target attempt | expected local environment block: `x86_64-linux-gnu-gcc` is not installed; hosted Linux stable/MSRV passed for the merged SHA | `9c121d9eb1cb84fa3bcc1088474797c5ce49947d9f2b30c8b3e63d53f58f5af3` |

The locked-session Keychain lifecycle test requires explicit device Keychain
authorization and remains reported as ignored/unavailable. No path, display
name, account data, credential, or capture key is retained in this evidence.

## Live GitHub reconciliation

After issue #19 was updated with the acceptance receipt and closed, an
independent authenticated read reported 160 repository issues (114 open, 46
closed), 12 milestones, and 31 managed labels (45 labels total). Issue #19 is
closed and carries `status:done`; the other managed status counts are 114
`status:backlog` issues and no unexpected status labels.

The repository metadata planner was run twice against the public API with the
same task-tree digest
`5068e89fc5d34f586ce3a935d0a7c41f028ff35252e8bdcaae7f00200836c4e4`. Both
plans returned zero operations, the same plan digest
`5fa4dc93301002d2daa55fce5fdaad9060b7ceee0d20584f38915ae70a5a22e4`, and
byte-identical JSON receipts (SHA-256
`5ab80a19b049aa336e484db9b3869b784a2a396a728fbdeab36ad7ffaf6a71ee`). No
metadata apply was needed after this zero-delta result.
