# Task 0048 evidence: reproducible developer and fixture toolchain

Status: complete.

Task 0048 binds the M0 developer inputs and synthetic fixtures to explicit
versions, sources, seeds, byte lengths, and digests, then provides a clean-machine
smoke procedure that exercises the complete fixture-only path.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0048-CODE-D7FE3C0` — `d7fe3c06c9cf61c02c3a34772bb4c3ae16bd9e33` |
| Protected CI-context fix | `GHOSTRACE-0048-CONTEXT-E51954E` — `e51954e48e7fd4bbff69b9d245722839aed7f6af` |
| Pull request | [#179](https://github.com/AlisinaDevelo/GHOSTRACE/pull/179) |
| Protected-main merge | `GHOSTRACE-0048-MERGED-EB7FFB4` — `eb7ffb492a504f163a5952db0c03af70e917582c` |
| Post-merge verification log | `GHOSTRACE-0048-POSTMERGE-LOG-1BB4FA` — SHA-256 `1bb4fa9cf4a12ebf156bf190f96e446d8ff859994c34e62d6e255d88edc76f5b` |

The raw log is retained locally only. The captured hardware inventory redacts
serial number, UUID, UDID, username, and computer name.

## Acceptance mapping

1. **Pinned toolchain and install sources.** `rust-toolchain.toml` pins Rust
   `1.88.0` with the minimal `clippy` and `rustfmt` components. `Cargo.lock` is
   required for locked commands and its SHA-256 is recorded in
   `toolchain/manifest.json`. The CI checkout and Rust setup actions remain
   commit-pinned, and every CI Rust job requests `1.88.0` while retaining the
   protected check names. The standard-library-only Python checks require Python
   3.9 or newer.
2. **Synthetic fixture provenance.** `fixtures/manifest.json` binds all three
   checked-in synthetic fixtures to generator metadata
   `ghostrace-fixture-manifest-v1`, seed `ghostrace-fixture-seed-v1`, format,
   byte length, and SHA-256. Its privacy declaration is synthetic-only,
   user-data-free, and offline; `scripts/fixture-manifest.py check` rejects
   drift, unsafe paths, and changed metadata.
3. **Clean-machine smoke.** `scripts/reproducibility-test.sh` installs no
   dependencies and runs with `CARGO_NET_OFFLINE=true`. It verifies the pinned
   inputs and fixture manifest, Rust formatting, schema equivalence, byte-stable
   demo and export output, capture refusal, roadmap validation, all Python tests,
   locked Clippy, and all-target/all-feature Rust tests.

## Exact merged-main verification

Target: MacBook Pro `MacBookPro17,1`, Apple M1, 8 cores, 8 GB RAM; macOS
`26.6.2 (25G83)`, Darwin `25.6.0`, `arm64`; `rustc 1.88.0
(6b00bc388 2025-06-23)`, Cargo `1.88.0 (873a06493 2025-05-10)`, host
`aarch64-apple-darwin`.

The post-merge log records the exact source SHA `eb7ffb492a504f163a5952db0c03af70e917582c`:

- `scripts/reproducibility.py check` and `scripts/fixture-manifest.py check` — pass;
- `cargo +1.88.0 fmt --all -- --check` — pass;
- schema output compared structurally to `schemas/event-envelope-v1.json` — pass;
- repeated demo output and repeated export output — byte-identical;
- `capture` — refused with the intentional live-capture gate;
- `python3 scripts/roadmap.py check` — pass: 160 tasks, 12 milestones, 488 dependency edges;
- `python3 -m unittest discover -s tests -p 'test_*.py'` — pass: 33 tests;
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` — pass;
- `cargo +1.88.0 test --locked --all-targets --all-features` — pass: 20 vertical-slice tests, 1 privacy regression, 5 support-matrix tests, and the ignored canary outside its denial lane;
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec`: enforced canary, privacy fixture, and complete offline product suite;
- `git diff --check` — pass.

The local smoke and sandbox results are the acceptance evidence. GitHub Actions for
PR #179 were green and protected main required them for merge, but hosted Actions
are not substituted for this target-local reproduction. No user journal, export,
path, account name, credential, or network service was used.
