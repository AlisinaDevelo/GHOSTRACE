# Task 0057 evidence: hardened persistent journal path creation

Status: complete.

Task 0057 closes the local file-backed journal path boundary. Directory creation is
component-wise and user-owned; database, SQLite sidecar, temporary, backup, and
export artifacts are checked as regular, single-link, current-user-owned files with
restrictive modes. No live collector, network client, or privileged permission was
enabled.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `15c2c9755d53377163f6549bcc13cb0b89d302f8` |
| Implementation pull request | [#196](https://github.com/AlisinaDevelo/GHOSTRACE/pull/196) |
| Protected-main merge | `817570770ff80a02ab410f47da22a045dda18830` |
| Cargo.lock SHA-256 | `0c5de10ae5006ba3c1fe18f156831e2850af7065081ec2b80a3059f1729aa685` |
| Source-SHA local pipe log | SHA-256 `4ae50bef35e367ac14df9359558c540b3e45acf3635fd0110d6fa14c5d571964` |
| Merged-SHA local pipe log | SHA-256 `5b608d89e5a2c1c191e11574f1b59ec6cd3ee1805ef6ac757661e62e0509f930` |
| MVP demo explanation | synthetic fixture event `00000000-0000-4000-8000-000000000008`; JSON SHA-256 `8e4e78c49f923a2ad6631e012cd7be17f6fc08b65887c171f2a88d4062deeddb` |
| MVP demo export | 8-event JSONL export; SHA-256 `fd47b9b1ba689934748605f1ed50f950a2b3f7da0fca6687e1ade1f4e5a201d5` |
| MVP capture refusal | bounded disabled-capture stderr; SHA-256 `9c30a3395e36f5245b81ad296212fea250f05934ebd8163d4c9bbdb5abef48da` |

Raw logs and demo files are retained locally in `/private/tmp` only. They contain
synthetic fixture output and test results, not a real journal, user path, account,
credential, key, or network service data.

## Acceptance mapping

1. **Unsafe creation and identity.** `src/storage.rs` rejects target symlinks,
   non-directories, non-regular files, foreign ownership, hard-link counts other
   than one, `..` traversal, and group/world mode bits. Existing path components are
   checked without following attacker-controlled links; the macOS root-owned
   `/var`, `/tmp`, and `/etc` compatibility aliases are the only explicitly allowed
   system aliases. New files use no-follow opens and `create_new`.
2. **Restrictive artifacts.** Journal directories are exactly `0700`. The database,
   WAL, SHM, rollback-journal, temporary, and backup sidecars are verified as
   `0600`; the same mode is applied to temporary and final exports. A forced export
   may repair the mode of a single-link regular file but refuses symlink, hard-link,
   foreign-owned, and non-regular destinations. File-backed migration and every
   committed ingest recheck database/sidecar metadata.
3. **Race refusal.** The storage unit test replaces the journal directory with a
   symlink between initial validation and the secure open, then asserts a bounded
   `UnsafePath`/`PathRace` refusal and no outside database creation. Handle/path
   device-inode checks and parent rechecks cover replacement after the no-follow
   file open as well.

## Local verification on the merged SHA

Target device: MacBook Pro 17,1, Apple M1, 8 GB, `arm64`; macOS 26.6.2 (25G83);
Rust/Cargo 1.88.0 (`aarch64-apple-darwin`). Exact source under test:
`817570770ff80a02ab410f47da22a045dda18830`.

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `cargo +1.88.0 check --locked --offline --all-targets --all-features` — pass.
- `cargo +1.88.0 clippy --locked --offline --all-targets --all-features -- -D warnings` — pass.
- Focused storage tests — pass: 4 tests covering symlink/non-regular/hard-link
  refusal, unsafe modes, sidecar modes, and parent replacement.
- `cargo +1.88.0 test --locked --offline --all-targets --all-features` — pass:
  6 unit tests, 1 privacy regression, 5 support-matrix tests, and 26 vertical tests.
- `cargo +1.88.0 test --locked --offline --doc --all-features` — pass (0 doctests).
- `cargo +1.88.0 test --locked --offline --release --all-targets --all-features` — pass:
  6 unit tests, 1 privacy regression, 5 support-matrix tests, and 26 vertical tests.
- `cargo +1.88.0 doc --locked --offline --no-deps --all-features` — pass.
- `scripts/reproducibility-test.sh` — pass: pinned inputs, schema, deterministic
  demo/export, capture refusal, 38 Python tests, roadmap, fixture/identity checks,
  clippy, and locked all-target tests.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`, including
  the enforced denial canary, privacy regression, and complete product suite.
- `shellcheck scripts/*.sh`, `actionlint`, `python3 scripts/release-evidence.py check`,
  and `git diff --check` — pass.
- Resource sample for the merged-SHA all-targets test: 3.34 s real, 193,314,816
  bytes maximum resident size, and 43,124,032 bytes peak memory footprint. This is
  a single local smoke measurement, not a production capacity claim.

The local MVP demo on the merged SHA produced a deterministic 8-event causal
explanation and JSONL export; `capture` refused with the bounded message that live
capture remains disabled until its policy/cursor/Keychain gates land.

`cargo-audit` and `cargo-deny` were unavailable on this device. No hosted check is
used as acceptance evidence; repository checks on PR #196 were only protected-branch
merge gates. Intel macOS, older macOS releases, signed/notarized distribution,
Keychain round-trip under a signed entitlement, and live collectors remain explicit
no-go/unsupported claims until separately tested.
