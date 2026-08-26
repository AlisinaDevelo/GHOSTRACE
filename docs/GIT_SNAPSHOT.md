# Metadata-only Git snapshot contract

Task 0095 defines the privacy boundary for a future explicitly requested Git
snapshot. The contract is implemented by `GitSnapshotMetadata` and is not a Git
command runner. It accepts normalized facts only; it has no path, ref name,
remote, command, object-reader, or file-content input.

## Retained facts

Each snapshot contains:

- an opaque `repository_id` and repository kind;
- an explicit `sha1` or `sha256` object format;
- optional algorithm-tagged IDs for the HEAD commit, tree, and index;
- a worktree state, branch class, and bounded operation class;
- bounded staged, unstaged, untracked, and conflicted counts; and
- explicit source limitations for partial clones, replace refs, shallow history,
  submodules, and alternate object databases.

Object IDs are metadata, not content. `GitObjectIdRef` accepts only
`sha1:<40 lowercase hex>` or `sha256:<64 lowercase hex>`, and the declared
algorithm must match every ID in the snapshot. The contract never opens an
object or resolves an ID to a tree, commit, blob, filename, or patch.

## Excluded baseline data

The type has no representation for ref names, commit messages, authors, remote
URLs, credential helpers, config values, reflog messages, diffs, patches,
filenames, untracked content, arguments, or raw paths. Unknown JSON fields are
rejected before a snapshot is accepted, and boundary errors never echo the
rejected value. The existing synthetic event-envelope Git row is a fixture for
the earlier envelope contract; it is not authorization for a live adapter to
retain a branch name or any other excluded field.

## Limits and continuity

The serialized metadata is capped at 16 KiB. Each status class is capped at
1,000,000 entries and the combined count at 4,000,000. Clean snapshots must
have zero status counts; bare repositories must use `not_applicable` and
`no_worktree` with zero counts. A SHA-256 digest binds the canonical metadata
fields and is checked on parse.

Every source limitation is required. `unknown` is a valid explicit result when
the adapter cannot establish whether a partial clone, replacement ref, shallow
boundary, submodule, or alternate object database is present. The adapter must
not silently report a complete history in that case. These limitations describe
coverage; they do not claim that an object ID or status count proves intent or
file-level causality.

## Adapter rule

An adapter may inspect Git's metadata interfaces only after explicit user
authorization. It must normalize the small field set above, discard all other
strings and command output, and call `GitSnapshotMetadata::from_identity`. The
constructor performs no filesystem, Git, network, or object-database I/O, so
the default read policy is `metadata_only`. A later task may define an explicit
adapter, consent, event projection, and failure/gap semantics; this contract
does not ship one.

The checked-in schema and golden example are
[`schemas/git-snapshot-metadata-v1.json`](../schemas/git-snapshot-metadata-v1.json)
and [`fixtures/git-snapshot-metadata-v1.golden.json`](../fixtures/git-snapshot-metadata-v1.golden.json).
The focused tests cover algorithm mismatch, malformed IDs, unknown/excluded
fields, status and bare-repository bounds, all operation classes, limitation
states, digest drift, oversized input, and deterministic serialization.
