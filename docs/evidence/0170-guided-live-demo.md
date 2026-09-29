# Task 0170 evidence: guided live demo

Status: implementation, review, protected-main merge, and merged-main device
verification complete. Implementation PR [#401](https://github.com/AlisinaDevelo/GHOSTRACE/pull/401) was squash-merged to
protected `main` at `55a45795089bd63411b16006f5745bda86ff0d78`.

The deliverable is `scripts/live-demo.sh` and `docs/DEMO.md`.

## Contract and acceptance mapping

| Evidence | Acceptance criterion | Retained result |
|---|---|---|
| E-0170-01 | A script runs the demo in a temporary workspace and cleans it up, with no network and no personal data. | The script creates its own home, key, folder, and repository in a temporary directory, sets `GIT_CONFIG_GLOBAL=/dev/null`, contacts no network, and deletes the key, home, and workspace in an EXIT trap; `tests/test_live_demo.py` checks these properties (`live-demo.log`, exit 0). |
| E-0170-02 | The documented walkthrough shows real output from the reference device. | `docs/DEMO.md` is a run on the reference M1; the rerun in `live-demo.log` matches it apart from IDs, times, and FSEvents coalescing. |
| E-0170-03 | The demo states what was not observed and why (coverage limits and gaps). | Section 9 of the script lists what was not observed and why, and the run shows the `shell_exec_failed` and `history_rewritten` gaps and contextual renames. |

## Delivery

- Issue: [#384](https://github.com/AlisinaDevelo/GHOSTRACE/issues/384)
- Implementation PR: [#401](https://github.com/AlisinaDevelo/GHOSTRACE/pull/401), head before squash `b03c121c383ca97abbf60f3b70d95c72b2088bd7`, merge `55a45795089bd63411b16006f5745bda86ff0d78`
- Verification date: 2026-09-29 UTC

## Device and toolchain

```text
Darwin 25.6.0
26.6.2
MacBookPro17,1
Apple M1
8
arm64
rustc 1.88.0 (6b00bc388 2025-06-23)
host: aarch64-apple-darwin
cargo 1.88.0 (873a06493 2025-05-10)
Python 3.9.6
verified source revision: 00e0dc773b13daa294b9397105c43b81d8f3df2d
```

## Merged-main device verification

Every command ran from protected `main` at `00e0dc773b13daa294b9397105c43b81d8f3df2d` on the reference device, with
a private Cargo target directory and `GIT_CONFIG_GLOBAL=/dev/null`. Hosted checks are
corroboration; the retained device logs are the acceptance evidence.

- `bash scripts/reproducibility-test.sh`, `cargo +1.88.0 clippy --locked --all-targets
  --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo +1.88.0 doc --locked
  --no-deps`, and `cargo +1.88.0 build --release --locked` (with and without
  `--all-features`) exited `0`.
- `cargo +1.88.0 test --locked --all-targets --all-features -- --skip
  macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt` exited `0`:
  382 passed, 0 failed, 3 ignored.
- `python3 -m unittest discover -s tests -p 'test_*.py'` exited `0`: 73 tests.
- `GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 cargo +1.88.0 test --release --locked --all-features
  --test login_keychain --test live_cli` exited `0`: 2/2 and 2/2.
- `GHOSTRACE_FRONTMOST_TEST=1 cargo +1.88.0 test --release --locked --features frontmost
  --test frontmost_macos --test frontmost_identity` exited `0`.
- `device-checks.sh` and `scripts/live-demo.sh` ran against the optimized
  `--all-features` binary and exited `0`. A first device-check run used a binary that a
  later feature-specific test build had replaced; it stopped at the archive step and is
  retained under its own name.

## Retained artifacts

Logs are retained on the verification device in `~/Developer/dev/ghostrace-evidence`,
outside the repository and outside temporary storage.

| Artifact | Result | SHA-256 | Bytes |
|---|---|---|---:|
| `ghostrace-evidence/merged-00e0dc7/device-info.txt` | exact device/toolchain capture | `b7723aa8216b7b867cf2402518c0106f93c330786ea76c51e6579e84a022ca34` | 232 |
| `ghostrace-evidence/merged-00e0dc7/repro.log` | reproducibility lane, exit 0 | `38820862cdc6d183786f9e17d2b523a743df5cde56c77a7127b5ff7e6dc042e5` | 61621 |
| `ghostrace-evidence/merged-00e0dc7/clippy.log` | Clippy with warnings denied, exit 0 | `2e50eba1ed57adc5f3d07f377702d6e438a23ebd1b3d424eb36c6677a9d323a9` | 60 |
| `ghostrace-evidence/merged-00e0dc7/all-targets.log` | all targets except native benchmark, 382 passed | `d8983ca62f19352820196582152066e2abb1355a0cf579d0cf3fe31f2e54f8fb` | 48122 |
| `ghostrace-evidence/merged-00e0dc7/rustdoc.log` | rustdoc with warnings denied, exit 0 | `0038e008d6fcdfd95721afdd77def13671f6bb396e0a0cb2dc788b55d347be7e` | 1055 |
| `ghostrace-evidence/merged-00e0dc7/release-build.log` | optimized release build, exit 0 | `07af3cf1276fc49ba3baec095e7ea24bef7afc3ce4d4eefe3b21c5809a4a0852` | 3439 |
| `ghostrace-evidence/merged-00e0dc7/release-build-all-features-rebuild.log` | optimized --all-features build used by the scripts, exit 0 | `d1235ea4563620b53ef5bdd2da5f595fec9d54f9efb48376378c20547dd95e57` | 62 |
| `ghostrace-evidence/merged-00e0dc7/python.log` | Python suite, 73/73 | `143fd0af1bf19eb57cbe2d5f43a62aefd7100fc91d2ffbd14b11401cb97c2ca2` | 172 |
| `ghostrace-evidence/merged-00e0dc7/release-keychain-live.log` | gated live_cli 2/2 and login_keychain 2/2 | `49162e77c6372c58eff9b31ab3c152ee80c4c0e62a5bfc30592055350e84ff88` | 2629 |
| `ghostrace-evidence/merged-00e0dc7/release-focused.log` | authenticated_state 9/9, parquet_archive 4/4, timeline_report 3/3 | `04ea7241658c9584d63314b5263db25dd748f8777ac0e14a98d88fee6f4e46e7` | 2040 |
| `ghostrace-evidence/merged-00e0dc7/release-frontmost.log` | frontmost_identity 8/8 and the frontmost_macos checks including a real focus switch | `430554f87cbbc13102a3057e3afe6fa84f4ba2249ba41b46bd8c2082c7a98b5e` | 1806 |
| `ghostrace-evidence/merged-00e0dc7/device-checks.sh` | device check script | `3e6972848ae88a3be55a5a6c510e959ee3a7df42d2d4dbcf5a5824c30c49c760` | 3394 |
| `ghostrace-evidence/merged-00e0dc7/device-checks.log` | device checks, exit 0 | `5f683ddd2096a29e9f171e40df94d58a5c506da7c920cdf42004fa035130eb2c` | 2789 |
| `ghostrace-evidence/merged-00e0dc7/device-checks-frontmost-only-binary.log` | first device-check run, against a binary rebuilt without the parquet feature; exit 1 at the archive step | `904b608a3476e3bc34014e534c78ee7742446b55cf9a065b60fd808f0d728546` | 2848 |
| `ghostrace-evidence/merged-00e0dc7/live-demo.log` | scripts/live-demo.sh, exit 0 | `ab55bca27e41990d7cf37b980f7a9b6707fc935859fbc2da819c695a9c3a0d0b` | 8308 |

## Privacy, failure, and scope boundaries

- The demo needs the login keychain and so runs on macOS only.
- An earlier draft printed a git warning naming the home directory's template path; the script now ignores the user's Git configuration.
