# Task 0173 evidence: live export and archive

Implementation and acceptance verification are complete. The live export was
implemented in PR [#406](https://github.com/AlisinaDevelo/GHOSTRACE/pull/406), merged
at `340a0e446da096acb90992609c66f08274a46a04`. Acceptance tests were reviewed and
merged in PR [#409](https://github.com/AlisinaDevelo/GHOSTRACE/pull/409) at
`3248b3b6fc1ee1771f10a26ab2ad2090fa483b16`. The candidate and merge trees matched.

Issue: [#394](https://github.com/AlisinaDevelo/GHOSTRACE/issues/394).

## Acceptance mapping

| Evidence | Criterion | Merged-main result |
| --- | --- | --- |
| E-0173-01 | Same preview, plan/snapshot confirmation, home key custody, and private atomic publish. | The actual login-keychain CLI runs interactive decline and affirmative confirmation against identical destinations. Decline creates no JSONL, Parquet, or temporary output. Both previews have identical plan/snapshot digests and event counts; the manifest count and event-body digest match the confirmed snapshot. JSONL and Parquet modes are 0600 and no temporary output remains. The five fixture preview tests also cover changed policy/snapshot refusal and cleanup. |
| E-0173-02 | JSONL validation and Parquet archive verification. | Interactive and concurrently-watched exports pass `ghostrace validate`; both archives pass `verify-archive` against their source exports. The `parquet` feature was enabled during the device run. |
| E-0173-03 | Home destination refusal and running-watch suppression. | Both interactive and automated tests refuse destinations inside the home and refuse existing outputs. The actual FSEvents watch records 0 changes for exports, archives, and reports produced during its run. A separate test confirms ordinary outside-home file writes are still observed. |

## Exact device reproduction

Verification date: 2026-10-02 UTC. Source revision:
`3248b3b6fc1ee1771f10a26ab2ad2090fa483b16`.

Reference device: MacBookPro17,1, Apple M1, macOS 26.6.2 (25G83), arm64.
Toolchain: rustc 1.88.0 (`6b00bc388`, host `aarch64-apple-darwin`), cargo 1.88.0
(`873a06493`), Python 3.9.6.

```sh
GHOSTRACE_LOGIN_KEYCHAIN_TEST=1 cargo +1.88.0 test --locked --features parquet \
  --test live_cli --test login_keychain --test export_preview \
  -- --test-threads=1 --nocapture
```

Exit 0: five live CLI tests actually ran, two keychain tests passed (including the
real provision/read/encrypted-reopen/delete round trip), and five fixture preview
tests passed. There were no skipped live tests in this feature combination. The
throwaway homes and keychain items were deleted; no production home or permanent
signing identity was used. Both previews use the same CLI executable/keychain ACL.

The complete reproducibility lane also passed on protected main
`7b64cadb1bb4ea352f5a9c13f3675f4a6bdacf7d`, which has the identical production
export code. That lane covers deterministic exports, retention, authentication,
verified-copy recovery, all 73 Python tests, Clippy, and the full Rust suite except
the separately tracked native benchmark. It is corroboration, not a substitute
for the opt-in merged-main device run above.

## Retained artifacts

Logs are retained privately on the verification device outside the repository,
under audit ID `ghostrace-audit-20261002` in `ghostrace-evidence/audit-20261002.aaOsOD`.
No journal, plaintext export, private path, or key is committed.

| Artifact | Result | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| `export-merged-native.log` | Exact merged-main 12-test device run, exit 0 | `27e216398643831b5f1fa1de1deb51b877f6f468fc31151ef40a96664a2e52ac` | 1547 |
| `environment.txt` | Device, OS, architecture and pinned toolchain capture | `94d4a8db3508c48249b0c698931783fa1ca16ce53d4627fe10fd4ce6bc565ff2` | 448 |
| `merged-reproducibility.log` | Protected-main reproducibility lane, exit 0 | `5780608a6510629bba4daf50610f5927e5163c7603c395e336d2f15b76837d23` | 52222 |

## Limits and no-go results

- This export is an explicit plaintext declassification, not an encrypted backup
  or a guarantee that external copies can be erased.
- Device evidence covers macOS login-keychain custody. It does not establish
  Developer ID/data-protection-keychain, signing-rebuild approval, locked-session,
  sleep/wake, or notarization behavior.
- The timing-sensitive native filesystem benchmark is tracked separately; this
  task adds no large-journal throughput or release-readiness claim.
