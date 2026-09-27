# Compatibility matrix

GHOSTRACE version fields are only meaningful if compatibility behavior is
exercised continuously. [`planning/compatibility-matrix.json`](../planning/compatibility-matrix.json)
lists every retained public format, the versions its readers accept, the
version its writers emit, a golden artifact, and an explicit outcome for each
compatibility case:

| Case | Meaning | Required outcome |
|---|---|---|
| `current` | The golden artifact of the current version | `accept` |
| `backward` | An older supported version | `accept`, `accept_with_upgrade`, or `refuse`, stated per format |
| `forward` | A newer version than the reader knows | `refuse` |
| `unknown_field` | A field the strict contract does not define | `refuse` |
| `mixed` | Records of different versions in one export | `refuse` |
| `corrupted` | Truncated, malformed, or checksum-mismatched input | `refuse` |
| `partially_migrated` | A journal whose migration ledger is incomplete | `refuse` |
| `downgrade` | Opening a newer journal with an older reader | `refuse` |

Every outcome names the test that proves it. `python3 scripts/compatibility.py
check` runs in the required `roadmap` CI job and fails when a required case is
missing, an unsafe case is accepted, a golden artifact is missing, a writer does
not emit the current version, or a named test function does not exist.

`tests/compatibility_matrix.rs` carries the proofs that were not already covered
elsewhere. Each v1 JSON contract (event envelope, Git snapshot, shell metadata)
accepts its golden and refuses forward, backward, unknown-field, and corrupted
variants. The export stream refuses forward, backward, unknown-field, mixed,
corrupted, and truncated input. A journal created at migration 1 is upgraded,
ingested, queried, explained, exported, integrity-checked, pruned through the
confirmed retention path, and reopened. Partially migrated and future journals
are refused.

## Removing compatibility

A format moves to `status: deprecated`, or a version is added to `retired` and
removed from `supported_read`, only with a deprecation record naming:

- the release that announced it (`announced_in`);
- the earliest removal release (`removal_not_before`), at least the policy's
  minimum window of releases later;
- an existing migration tool;
- existing rollback evidence;
- the release-note impact statement.

The checker refuses a deprecation or retirement missing any of these, and a
retired version that is still listed as readable.
