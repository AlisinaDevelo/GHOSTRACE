# Task 0012 evidence: fixture ingest, explain, and JSONL export CLI

Status: complete on protected `main`; implementation merge `30172a2a51ddaadea8dcdf7cc6d0d2d8361d6ec0`
(PR #235) and final evidence readback `4cd1d7eef52ac03b31cc785064ad074108b357a6`
(PR #236). This receipt proves a restartable fixture-only CLI path without enabling
live collectors.

## Contract and implementation

The durable path is:

```text
init --journal <private path>
  -> ingest --journal <path> --fixture <JSONL>
  -> explain --journal <path> --event <UUID>
  -> export --journal <path> --output <JSONL>
```

`init` is idempotent and uses the hardened file-backed SQLite opener. `ingest`
validates the checked-in fixture through the typed fixture origin and deny-by-default
policy, then commits the batch before reporting success. `explain` and `export`
reopen the journal in separate processes with the same synthetic fixture key. The
existing `demo --fixture` and `export --fixture` shortcuts remain available for
in-memory fixture checks. Exporting over the source journal is refused even with
`--force`.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Init, fixture ingestion, deterministic explain, and versioned JSONL export work end to end | `tests/cli_vertical_slice.rs::durable_fixture_cli_path_is_reopenable_deterministic_and_capture_disabled` initializes the journal twice, ingests all 8 fixture events, explains the terminal event twice with byte-identical output (8-event chain and one gap), and exports a version-1 manifest plus 8 event records. |
| Live collectors are refused in fixture-only mode | The same black-box test requires `capture` to exit non-zero with the documented refusal. The full reproducibility lane repeats the refusal under `CARGO_NET_OFFLINE=true`; no collector or network path is enabled by this change. |

## Device receipts

Receipts were run on 2026-08-25 against protected-main merge SHA
`30172a2a51ddaadea8dcdf7cc6d0d2d8361d6ec0` on a MacBookPro17,1 (Apple M1, arm64),
macOS 26.6.2 build 25G83, Darwin 25.6.0, Rust/Cargo 1.88.0,
`aarch64-apple-darwin`, and Python 3.9.6.

| Lane | Result and retained receipt |
| --- | --- |
| Protected-main focused durable CLI test | Pass: 1 test; `/tmp/ghostrace-0012-postmerge-cli.log`; SHA-256 `9bfd619c71925b2174c4fc9bbd96413b16f253ed091f430d5b21307f07e71969` |
| Protected-main reproducibility/device pipe | Pass: pinned inputs, schema, demo, durable init/ingest/reopen/explain/export, capture refusal, 40 Python tests, locked Clippy, and all Rust targets; `/tmp/ghostrace-0012-postmerge-repro.log`; SHA-256 `bf7f84602865cd671b8a50bc6c97c2b852b665d6d01d786bdf0dbd27c83f6af0` |
| Final protected-main focused readback at `4cd1d7e` | Pass: 1 test; `/tmp/ghostrace-0012-final-main-cli.log`; SHA-256 `059c0648a03c2be2601dcdc6a6dfb36d97719b01bf71800cbc8381cc4c3d7d33` |
| Final protected-main reproducibility readback at `4cd1d7e` | Pass: same complete local pipe; `/tmp/ghostrace-0012-final-main-repro.log`; SHA-256 `77e6c4911890f89fdedbbf39cc5756559fca34b33019b3782c97ad7457cd17a7` |
| Implementation commit before merge | `ae5924013bd61f63fbd99e9e929fe2e71aa7934d` on PR #235; local full pipe passed before push |
| Hosted merge gates | PR #235 passed roadmap, audit, dependency review, deny, fixture network-denial, rustfmt, Clippy, and Linux/macOS/MSRV test lanes before merge |

The receipts are local device evidence; hosted checks are an additional merge gate,
not a substitute for the reproduction.

## Scope limits

The CLI path remains fixture-only and offline. Its deterministic synthetic key is
not a production encryption or key-management claim. This task does not ship live
FSEvents, shell, Git, frontmost-app, or browser collectors; it does not request
permissions, upload data, validate Intel hardware, or claim signed/notarized release
artifacts. Duplicate fixture ingestion is rejected by the journal's idempotence and
conflict rules rather than silently duplicating rows.

## Closure

Issue #16 can be closed against this receipt. Future live collection and production
Keychain integration remain separate roadmap gates.
