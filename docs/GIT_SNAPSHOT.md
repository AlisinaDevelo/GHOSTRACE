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
the default read policy is `metadata_only`. The explicit adapter below is the only
component that runs Git; event projection into the journal is a later step.

The checked-in schema and golden example are
[`schemas/git-snapshot-metadata-v1.json`](../schemas/git-snapshot-metadata-v1.json)
and [`fixtures/git-snapshot-metadata-v1.golden.json`](../fixtures/git-snapshot-metadata-v1.golden.json).
The focused tests cover algorithm mismatch, malformed IDs, unknown/excluded
fields, status and bare-repository bounds, all operation classes, limitation
states, digest drift, oversized input, and deterministic serialization.

## Explicit snapshot adapter

`GitSnapshotAdapter` (`src/git_adapter.rs`) takes a snapshot only when a caller
asks for one repository under a policy that enables the `git` source and selects
the named root. It runs a fixed set of read-only plumbing commands and reduces
their output to the contract above:

- `rev-parse` for bare/shallow state, object format, and the common directory,
  git directory, and top level (used only to read device/inode identity);
- `rev-parse --verify` for the HEAD commit and tree IDs;
- `symbolic-ref --quiet HEAD`, of which only the `refs/heads/`, `refs/remotes/`,
  or `refs/tags/` prefix is inspected to derive the branch class;
- `status --porcelain=v2 -z`, whose NUL-separated records are counted as staged,
  unstaged, untracked, or conflicted (a rename counts once);
- `for-each-ref refs/replace/`, promisor and partial-clone configuration, the
  alternates file, `.gitmodules`, and the superproject link for the limitations;
- marker files in the git directory for the operation class.

Ref names, paths, filenames, and command output never leave the adapter, and
errors are fixed strings. Repository configuration is treated as hostile: every
command runs with a cleared environment, no system or global configuration,
`--no-optional-locks`, no lazy promisor fetches, and `-c` overrides that disable
fsmonitor, the untracked cache, hooks, pagers, credential helpers, and every
transport. Each configured `filter.*.clean|smudge|process` driver is overridden
with an empty command before `status`, so a racily modified file cannot run a
repository-supplied filter. Output is capped at 64 MiB and each command at 20
seconds. `probe_ancestry` answers the single history question from `cat-file -e`
and `merge-base --is-ancestor` exit status.

`tests/git_snapshot_adapter.rs` exercises real throwaway repositories: status
counting with names absent from the output; hostile branch and file names
(command substitution, bidirectional override, newline, tab, emoji, leading dash)
including a rename with a newline; a repository whose fsmonitor, hook, filter
driver, and pager each create a marker under plain `git status` but not during a
snapshot; unborn, detached, merge-conflict, and bare states; shallow, partial,
shared-alternates, replace-ref, and submodule limitations; ancestry probes feeding
the history-transition contract; and policy, missing-repository, and missing-Git
refusals.

## Repository-local hook lifecycle

`GitHookManager` (`src/git_hooks.rs`) optionally installs a shim for
`post-checkout`, `post-commit`, `post-merge`, and `post-rewrite` in the
repository's own hooks directory (the common directory, so linked worktrees share
it). Each shim is a fixed script that runs the configured absolute delegate as
`<delegate> git-hook <name>` with standard input closed; hook arguments are not
passed. No global Git configuration is read or written.

| Operation | Behavior |
|---|---|
| `plan_install` | Lists the exact file and action (`create`, `replace`, `unchanged`) for every managed hook without writing. |
| `install` | Applies the plan under a lock, creating new shims with exclusive create and replacing only recorded, intact shims atomically; repeating it is a no-op. A changed delegate or shim version is an upgrade (`replace`). |
| `verify` | Reports each recorded shim as `intact`, `missing`, or `drifted` against its recorded SHA-256 digest and mode. |
| `disable` / `enable` | Clears or restores execute bits on intact shims; Git skips non-executable hooks, and content is untouched. |
| `uninstall` | Removes only shims whose content still matches the record, re-checking each immediately before removal, then the record. |

The manager refuses the whole operation, before writing anything, when a managed
hook already exists and is not a GHOSTRACE shim, when `core.hooksPath` is set in
repository or worktree configuration (another hook manager owns hooks), when the
hooks directory or a hook is a symlink or owned by another user, when any recorded
shim drifted, when another hook operation holds the lock, or when a file appears
during exclusive creation. The digest record is `hooks/ghostrace-hooks.json`.

`tests/git_hook_lifecycle.rs` runs each operation against real repositories and
checks that a real commit runs the delegate, that a disabled shim does not, that a
user hook keeps its content and mode, that drifted shims survive uninstall, and
that a linked worktree resolves to the same shims.

## History transitions and gaps

Local Git history is mutable, so a later graph cannot prove what an earlier
snapshot could see. `GitHistoryTransition::classify` (`src/git_history.rs`)
compares two validated snapshots of the same repository and object format with
one bounded ancestry probe: whether the previous HEAD is an ancestor of the
current HEAD, reported only as `previous_is_ancestor`, `previous_not_ancestor`,
`previous_object_missing`, `shallow_boundary_reached`, or `not_probed`.

The result keeps three facts apart:

| Field | Source | Values |
|---|---|---|
| `ref_movement` | The two snapshots only | `unchanged`, `head_moved`, `detached`, `attached`, `became_unborn`, `first_commit`, `unknown` |
| `ancestry` | The probe, when no limitation could substitute the answer | `not_applicable`, `descendant`, `not_descendant`, `unknown` |
| `gap` | Any condition that loses or hides history | `history_rewritten`, `object_missing`, `shallow_boundary`, `replaced_objects`, `ancestry_not_probed` |

A gap carries the last known and current bounded state: HEAD object ID, branch
class, shallow, replace-ref, and partial-clone states. Ancestry is never inferred
from a missing object, a shallow boundary, an unprobed move, or an answer given
while replace refs are active. A moved shallow boundary or a missing previous
object is a gap even when HEAD did not move.

[`fixtures/git-history-transitions-v1.json`](../fixtures/git-history-transitions-v1.json)
defines the expected outcome for fast-forward, rebase, reset, force update, amend,
gc, object loss, shallow deepen, shallow-boundary walks, worktree detach, replaced
objects, unprobed moves, and a first commit. `tests/git_history_gaps.rs` also runs a
reference probe against throwaway repositories (fast-forward, amend, reset, gc
after reflog expiry, detach) using only `cat-file -e` and `merge-base
--is-ancestor` exit status with a cleared environment.
