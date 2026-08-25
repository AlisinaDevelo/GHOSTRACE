# Task 0005 evidence: privacy regression and network-surface checks

Status: complete for the fixture-only product contract. This gate does not enable
live collection, permissions, telemetry, cloud sync, or a network client.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Regression fixtures prove prohibited fields are absent | Child corpus `GHOSTRACE-0043-PRIVACY-CORPUS-V1` covers credentials, environment variables, command arguments, window titles, page content, clipboard text, and private-browser markers across ingest, errors, diagnostics, explanation, export, and CLI surfaces. The current merged-main debug and release focused runs below passed the same corpus. |
| Dependency and network policy is documented | [`docs/PRIVACY.md`](../PRIVACY.md), [`docs/THREAT_MODEL.md`](../THREAT_MODEL.md), [`docs/REPRODUCIBILITY.md`](../REPRODUCIBILITY.md), [`deny.toml`](../../deny.toml), [`Cargo.lock`](../../Cargo.lock), [`ADR 0004`](../adr/0004-offline-network-denial.md), and the checked-in offline workflow define the local/offline boundary, locked inputs, dependency policy, and explicit test-only network canary. The current product-surface scan found no network client or socket API; the only match is the denial canary in `tests/offline_network_canary.rs`. |
| Linux CI uses enforced network denial, or a checked-in equivalent decision record exists | Child lane `GHOSTRACE-0044-OFFLINE-CODE-477F56F` defines digest-pinned Linux Docker `--network=none` execution and its canary. The target Mac has no running Docker daemon, so [`docs/evidence/0044-offline-network-denial.md`](0044-offline-network-denial.md) records that limitation and the passing macOS `sandbox-exec` equivalent. The current merged-main sandbox lane passed below; no Docker result is claimed locally. |

## Current merged-main receipt

All current receipts below were run from protected `main` commit
`4a8f2493ec9955f2724a866fc562008cbd68b790` on 2026-08-25 on the target device:

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM from the pinned toolchain |
| Full debug offline lane | `/private/tmp/ghostrace-0005-merged-offline-v2.log`; SHA-256 `370d8ae79df5b1b1fa673798e40d876ba2805c6d46b966e7a857a78e1df93c23` |
| Focused debug privacy corpus | `/private/tmp/ghostrace-0005-merged-privacy-v2.log`; SHA-256 `2b5f5746ba0b8c36cf33560dd9ca1129559914766b7b07fbaf3b86c8e4e58299` |
| Focused release privacy corpus | `/private/tmp/ghostrace-0005-merged-release-privacy-v1.log`; SHA-256 `da287604b7356508e5584df26279d72bf8314c21e9e3f212acdda3dd1da74191` |
| Full release all-target suite | `/private/tmp/ghostrace-0005-merged-release-all-v1.log`; SHA-256 `a4a4cc6d0c054639dfc0ee68ab7fde3c5b1637e28a0004816564ae4ca4dbf876` |
| Static/policy receipt | `/private/tmp/ghostrace-0005-merged-static-v2.log`; SHA-256 `8cad25fa788fe0cadcd2d63e1ff66fbec2cd22489540f57b3cadf24fc6206d39` |

The enforced local lane passed the canary, the privacy fixture/explanation/export
test, and the complete locked debug all-target/all-feature suite. It reported 16
library tests, 5 support-matrix tests, 4 fault tests, 1 privacy test, 26 vertical
slice tests, 7 migration tests, 6 WAL tests, and 5 writer tests, with the expected
ignored ordinary canary entry. The focused privacy corpus passed independently in
both debug and release. The full release all-target/all-feature suite exited zero;
its migration crash-injection children intentionally print failed child tests while
the parent verifies abort-and-recovery and passes.

The static receipt passed formatting, Clippy with `-D warnings`, fixture-manifest,
identity, release-evidence, roadmap validation, generated-index parity, 38 Python
tests, ShellCheck, actionlint, and the product network-surface scan. The scan
allows only the explicit test canary's `TcpStream`; no product source or dependency
declares a network client.

## Failure, privacy, and resource boundaries

- The unsandboxed network canary is a negative control and is not accepted as an
  offline pass; the enforced sandbox canary is the acceptance run.
- The local Docker CLI has no running daemon. Linux Docker enforcement is therefore
  an explicit unavailable-hardware/runner condition, covered by the checked-in
  workflow and the passing macOS equivalent rather than substituted by a claim.
- The first current-main debug compile hit the device's full-disk limit; its target
  directory was task-owned and removed, then the focused and complete runs passed
  with one build job and stripped debug output. The failed receipt is retained as
  `/private/tmp/ghostrace-0005-merged-offline-v1.log` and is not an acceptance pass.
- No real journal, export, path, account name, credential, private browser content,
  or live event was used. Live macOS capture, permissions, and production deployment
  remain later gates.

## Post-merge closure rerun

The evidence/documentation merge itself was then checked from clean protected
`main` at `c815bf9826092474678d69ce677b8119fd7c144e`. The merge contains only the
ledger and evidence documentation listed in PR #211; no product source, fixture,
workflow, or test input changed after the `4a8f249` receipt above.

| Receipt | Result | SHA-256 |
| --- | --- | --- |
| `/private/tmp/ghostrace-0005-postmerge-release-v1.log` | Release privacy corpus: 1 passed, exit 0 | `4e535f2b9c8116dbef35538687dfd67a20f98214b0d6632fe597972d0457b0f2` |
| `/private/tmp/ghostrace-0005-postmerge-static-v1.log` | Format, fixture/identity/release/roadmap checks, index parity, 38 Python tests, ShellCheck, actionlint, product network scan: pass, exit 0 | `80736fa3ebb6c7adf65f814ecab313ce4159685302d278d18e9de1c4c589cc04` |

This exact-SHA rerun closes the documentation merge without treating hosted CI or
the unavailable local Docker daemon as a substitute for the target-device receipt.
