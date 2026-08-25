# Task 0077 evidence: snapshot-consistent query pagination

Status: implementation, review, merge, and protected-main reproduction complete.

Implementation PR [#274](https://github.com/AlisinaDevelo/GHOSTRACE/pull/274)
was merged to protected `main` at
`bd49f1d0d23cded0b10220ee6922f34572b30bf3` on 2026-08-25. Its implementation
commit was `48c7123c6010408bc9c184843f4b9bdb2cd9952a`. This artifact retains the
acceptance evidence for issue #81; the issue is closed only after this evidence
is merged and linked.

## Contract and acceptance mapping

`Journal::query_page` captures the maximum committed `ingest_seq` in its first
read transaction. Every later page carries that upper bound in an encrypted
token and filters rows with `ingest_seq <= boundary`. The result order is
`observed_at ASC, ingest_seq ASC, event_id ASC`; the final two keys provide
deterministic tie-breaking without implying causality.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Page tokens bind query parameters, policy scope, schema version, snapshot boundary, and stable ordering keys. | `QueryRequest` includes policy ID/version, computed scope digest, optional source/kind/time filters, and bounded page size. The authenticated token also includes the query digest, event schema, storage schema, issue/expiry timestamps, ingest boundary, and last ordering key. `tests/query_pagination.rs` exercises changed page size and cross-profile reuse. | PASS |
| Expired, forged, cross-profile, or future tokens fail with bounded errors. | AEAD token forgery fails as `QueryTokenInvalid`; a 15-minute expired token is covered by the query unit test; changed profile/page parameters fail as `QueryTokenMismatch`; a future storage schema marker fails as `QuerySchemaChanged`. Error text never includes token bytes or request values. | PASS |
| Concurrent ingest, deletion, retention, and migration tests prove the documented snapshot semantics. | The file-backed integration matrix writes after page one and proves the new event is absent from every later page, deletes the tail row after the snapshot and proves it is not resurrected, and changes `PRAGMA user_version` to a future schema and proves the active token is refused. It also asserts no duplicate across all returned pages. | PASS |

The deletion case models a future retention operation by removing one row from a
private test database after page one. The product contract is absence, not a
fabricated tombstone; retention residue and audit semantics remain a separate
M3 task.

## Protected-main device reproduction

The exact focused matrix was rerun from a fresh detached worktree at protected
`main` `bd49f1d0d23cded0b10220ee6922f34572b30bf3` on the named device. Combined
stdout and stderr for each lane is retained in `/tmp` and hashed here.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1) |
| Rust/Cargo | rustc/cargo 1.88.0, host `aarch64-apple-darwin` |
| Python | 3.9.6 |
| Source | protected `main` `bd49f1d0d23cded0b10220ee6922f34572b30bf3` |
| Date | 2026-08-25 |

Commands:

```text
cargo +1.88.0 test --locked --test query_pagination -- --nocapture
cargo +1.88.0 test --locked --test query_pagination --release -- --nocapture
```

Both commands exited 0 and ran all three integration tests.

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| protected-main debug query matrix | 3 passed, exit 0 | `188a065899fd6c8c4e909d6a08ad7d52effdd778c108b9be6f9740091410c693` |
| protected-main release query matrix | 3 passed, exit 0 | `87e6c40ddfe2354f661b57d7626afb0ef47f15303b57ffb6c89755dc67f44d32` |

The matrix uses only the checked-in synthetic causal-chain fixture and a private
mode-0700 SQLite journal. It retains no paths, file contents, account names,
token plaintext, or network payloads.

## Local implementation pipe

Before merge, the implementation branch ran every lane below on the same device;
each exited 0. The temporary log directory was
`/tmp/ghostrace-0077-local-pipe.UA7RyH`.

| Lane | Log SHA-256 |
| --- | --- |
| `cargo +1.88.0 fmt --all -- --check` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| all-target Clippy with warnings denied | `9a30d54c8b16d7e49510b182028079bca120aeebb7d662e88f8566e6196a49f6` |
| all-target debug tests | `5850b4c09c76637231c0068fbce629a3e5c1edc91dcc97699e944632b698b306` |
| all-target release tests | `64e87eceff229952e7a98c7bf66f9c59f98c0bc9dc5b76039475cb2a2aac8d32` |
| release build | `121acbbbab0993f903c59e08951310618d36b5558e6e3ac109e3af9ebee317aa` |
| rustdoc | `03068fbb950d9e6fad481e090ca7fa3374fcb152e64162af23fe16f9f29d7100` |
| focused integration query tests | `c766e81dab83499159ceeee3adc9c6b5e728cb1b54755f3926060db8c653058e` |
| focused query expiry unit test | `54c8b5ad0f599173af777d4885aa917a3389054b240bf9918373a4bf04f422a6` |
| Python tests | `a78c97907df0261bde946aa90eed2f7c15c19dbcaf96181fc4561a378b788b2a` |
| roadmap validator | `5e59e27d94c79aa00d451348331e9a7a6f32656ffeda8cca3abfa01afcc6fb1d` |
| release-evidence validator | `82ff0862f41c422cc4ad89be1d9fb40f64e2fda7179a690ffadff32994edc3f7` |
| fixture manifest check | `bff8fb4531b2932ae572808ab57b81d18200bbb3fa9b5d09a53af1706bb7e73f` |
| filesystem benchmark contract check | `595d08e6fa07b666881639549d11d57cffed59805bdc6dbc31f315ba1613b821` |
| reproducibility lane | `f82712bb60f68b2a24c0c9f80ffee1b1ff49b361956262510f773e121f1671c0` |
| offline network lane | `b20c7eda2c2e2e99ad3d7d2521bc57de21b442832056ddd6e2faed98191e3f5e` |
| FSEvents sanitizer lane | `3e216e99f8b0bf9d9cf2b6a1a194c86ccd067a0e1e0b4c790bcb2913a8dddc39` |

The red focused compile log before implementation has SHA-256
`1c477205fa9972d97ec33459d9826184b4868f06f1cf77f4b3e75827779c387a` and
failed because `QueryRequest`, `Journal::query_page`, and the bounded token
error variants did not yet exist.

Hosted pull-request checks corroborated the merge, but the local device pipe and
protected-main reproduction above are the acceptance evidence.

## Boundaries and limitations

- The snapshot boundary is an upper bound over committed ingest sequence. A
  later write is intentionally excluded; a later deletion is intentionally
  absent. The API does not claim MVCC resurrection or retention audit.
- The query currently filters journal metadata and policy identity; encrypted
  payloads are decrypted only for returned rows. No path or content filter is
  introduced.
- The token lifetime is 15 minutes and token encryption uses the active journal
  key generation. Key rotation or unavailable keys therefore fails closed.
- The migration test marks a private test database with a future schema using
  SQLite directly; it does not claim that production migration code permits a
  future schema. It proves the query reader refuses it.
- This task does not close the M3 aggregate gate or claim gap-aware windows,
  retention residue, explanations, or export semantics.
