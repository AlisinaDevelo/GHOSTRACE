# Task 0046 evidence: semantic identifier and digest contracts

Status: complete.

Task 0046 freezes the event v1 contract before live or third-party data is
accepted. Identifier-shaped fields now have explicit ASCII encodings, byte
bounds, rejection rules, and sensitivity classifications in
[`docs/EVENT_MODEL.md`](../EVENT_MODEL.md). Rust envelope and policy validation,
the checked-in JSON Schema, fixtures, and compatibility tests use the same
contract.

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Code before merge | `GHOSTRACE-0046-SEMANTIC-CODE-808D098` — `808d098753da885813f7402109df89b0cbee87d7` |
| Pull request | [#175](https://github.com/AlisinaDevelo/GHOSTRACE/pull/175) |
| Protected-main merge | `GHOSTRACE-0046-MERGED-F7D3EA0` — `f7d3ea0dfd5817be0b31931be8a9830478179e19` |
| Post-merge verification log | `GHOSTRACE-0046-POSTMERGE-LOG-62BBF0` — SHA-256 `62bbf0b71ea567ff697e72eec6d28ba803a56740617982d558bff03f677eb1a5` |
| Post-merge static verification log | `GHOSTRACE-0046-POSTMERGE-STATIC-038E4F` — SHA-256 `038e4fd209e0aac3fc333cfa654987c251eb3ba015a406fb005816ae7c45a6c3` |
| Fixture MVP demo log | `GHOSTRACE-0046-MVP-DEMO-8E4E78` — SHA-256 `8e4e78c49f923a2ad6631e012cd7be17f6fc08b65887c171f2a88d4062deeddb` |

The verification logs are retained locally only; their digests bind the exact
outputs without publishing device identifiers or untrusted test values.

## Acceptance mapping

1. **Typed contract, canonical encoding, bound, and sensitivity.** The event
   model table covers UUIDs, opaque IDs, reverse-DNS application IDs, Git branch
   names and object IDs, tagged SHA-256 digests, cursor tokens, reason codes,
   and sanitized URLs. The schema has matching definitions and every public
   event field references its semantic definition.
2. **Fail-closed constructors.** `EventEnvelope::new` and envelope
   deserialization reject absolute/relative paths, credential-shaped values,
   control characters, Unicode bidi/non-ASCII values, ambiguous encodings,
   invalid branch refs, non-canonical object IDs, and malformed/uppercase or
   wrong-length digests. Rejection messages identify only the field and contract
   class; they never echo the candidate.
3. **Schema, Rust, fixtures, compatibility.** The causal-chain fixture uses
   canonical tagged SHA-256 values. New integration tests exercise valid and
   invalid boundary values through both JSON Schema and Rust, verify non-echo
   behavior, and preserve the checked-in golden envelope serialization.

## Verification environment

- Device: MacBook Pro `MacBookPro17,1`, Apple M1, 8 cores, 8 GB RAM.
- OS: macOS `26.6.2 (25G83)`, Darwin `25.6.0`, `arm64`.
- Toolchain: `rustc 1.88.0 (6b00bc388 2025-06-23)`, Cargo `1.88.0
  (873a06493 2025-05-10)`, host `aarch64-apple-darwin`.
- Exact source under test: protected-main SHA
  `f7d3ea0dfd5817be0b31931be8a9830478179e19`.

The inventory intentionally records no serial number, hardware UUID, user
name, or computer name.

## Test matrix

All commands below were rerun from the exact merged SHA and are represented in
`GHOSTRACE-0046-POSTMERGE-LOG-62BBF0` unless noted otherwise:

- `cargo +1.88.0 fmt --all -- --check` — pass.
- `cargo +1.88.0 test --locked --all-targets --all-features` — pass: 20
  vertical-slice tests, 1 privacy regression, 5 support-matrix tests, and the
  ignored canary reported as ignored outside the denial lane.
- `cargo +1.88.0 test --locked --release --all-targets --all-features` — pass
  with the same test set.
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings`
  — pass.
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec` with
  `GHOSTRACE_OFFLINE_ENFORCED=1`: enforced canary, privacy fixture,
  explanation/export, and complete offline suite all passed with
  `CARGO_NET_OFFLINE=true`.
- `shellcheck scripts/offline-network-test.sh`, `python3 scripts/roadmap.py
  check`, `python3 -m unittest discover -s tests -p 'test_roadmap.py'`, and
  `git diff --check` — pass; see
  `GHOSTRACE-0046-POSTMERGE-STATIC-038E4F`.
- Fixture MVP demo:
  `cargo +1.88.0 run --quiet -- demo --fixture fixtures/causal-chain.jsonl
  --event 00000000-0000-4000-8000-000000000008` — pass; deterministic
  eight-event chain with one explicit coverage gap; see
  `GHOSTRACE-0046-MVP-DEMO-8E4E78`.

The invalid-value tests cover happy-path round trips, negative/path/credential/
control/ambiguous cases, non-echo privacy behavior, length/resource limits,
schema/Rust parity, deterministic replay, and failure-safe rejection. No live
collector, network, screen, audio, keylogging, or endpoint-security path is
claimed by this task.

GitHub Actions checks for PR #175 were green and protected main accepted the
merge, but Actions are not acceptance evidence for this task. Local `cargo-deny`
and `cargo-audit` binaries were unavailable on this Mac; hosted policy and
advisory gates passed, and that local tooling limitation is retained here.

## Follow-on boundary

The serialized Rust boundary remains `String` for wire compatibility; distinct
semantic newtypes and mutation/property coverage are the separately tracked M1
task 0050. This task freezes and enforces the contract at the envelope and
policy acceptance boundary so no live data can enter with the old free-form
semantics.
