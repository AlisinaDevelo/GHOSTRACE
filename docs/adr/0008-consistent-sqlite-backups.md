# ADR 0008: Consistent SQLite backups

- **Status:** Proposed
- **Date:** 2026-10-11
- **Scope:** Database-only backups and verified-copy repair

## Context

[ADR 0003](0003-sqlite-wal-active-journal.md) records the accepted WAL policy and
original checkpoint-then-file-copy mechanism. A journal's mutex serializes its
own writer and clones, but separately opened SQLite connections have independent
mutexes. A successful truncate checkpoint releases SQLite's checkpoint locks
before an ordinary file copy starts. A peer can then commit and checkpoint pages
while that copy reads the main file.

A synthetic regression atomically changes two distant rows and checkpoints from
a peer connection. The original backup returned success with different row
generations, a state no committed transaction contained. A deterministic copy
boundary reproduces the same result. Separately opening and closing the active
source file can also clear SQLite's process-owned POSIX locks.

## Proposed change

Keep the successful truncate gate, bounded busy wait, source artifact validation,
and database-only output contract. Replace ordinary source-file reads with
SQLite's backup API through the journal's existing connection. One complete
backup step holds a SQLite read snapshot while copying all pages. Success
requires `Done`; busy and locked results refuse without an unbounded retry loop.
The helper does not open or close another source descriptor.

Create a fresh destination with no-follow and exclusive-create semantics, set
mode `0600`, and refuse existing main or sidecar artifacts. Open that verified
file in SQLite with read/write and no-follow flags, without permission to create
another file. Use rollback journaling and `FULL` synchronous writes. Close SQLite
before checking the final identities and absence of sidecars, syncing the file
and directory, and closing the output descriptor. On supported Unix targets,
inspect the live SQLite destination with `SQLITE_FCNTL_HAS_MOVED` before pragmas
or backup writes, refusing a moved target or an unsupported/failed inspection.
A synthetic swap/restore regression otherwise reports success with an empty
named destination while writing another opened file. This check uses SQLite's
existing file descriptor rather than opening another path.
Failed copies remove a destination only when Unix file and parent identities
still match. Platforms without proven identities retain failed output.

Signed v1 checkpoints retain their exact-byte database identity, signature, and
verification behavior. SQLite backups rewrite bookkeeping bytes, so verified-copy
repair verifies the source checkpoint and the copy's own checkpoint, compares
all bound chain/schema/key/policy/integrity fields, and compares the full database
with only header offsets 24–27, 40–43, and 92–99 canonicalized. These contain the
change counter, schema cookie, and writer-version bookkeeping. Page count,
application/user versions, actual schema, and every page byte remain covered.
Both digests derive from the checkpoint's existing read buffer, without an
additional source descriptor. The manifest records the copy's own checkpoint;
the original source checkpoint verifies again after repair.

## Consequences and evidence

Concurrent commits or checkpoints can no longer produce a mixed-generation
backup. Backups remain separate private copies, and existing v1 receipts remain
valid for their original databases. A receipt for the source does not become an
exact-byte receipt for the rewritten copy.

The owner mutex and SQLite read snapshot last for the complete copy. Copy work
still scales with database size; this change establishes no new capacity or
elapsed-time guarantee. Repair hashing retains its existing full-file buffer.

Regression coverage includes deterministic peer checkpoints, a peer journal
commit followed by authenticated reopen, a bounded 32 MiB synthetic concurrency
fixture, busy cleanup, checkpoint starvation/retry, stale destination sidecars,
destination substitution during SQLite open, and v1 raw identity compatibility.
Byte mutation tests cover every position in a
4 KiB fixture and allow only the listed bookkeeping fields. No real journal,
new collection source, permission, or network client is involved.

Primary references: [SQLite backup API](https://www.sqlite.org/c3ref/backup_finish.html),
[SQLite file format](https://www.sqlite.org/fileformat.html#the_database_header),
and [SQLite's POSIX lock warning](https://www.sqlite.org/howtocorrupt.html#posix_advisory_locks_canceled_by_a_separate_thread_doing_close_).
The live-file inspection follows SQLite's
[file-control API](https://www.sqlite.org/c3ref/file_control.html) and
[HAS_MOVED contract](https://www.sqlite.org/c3ref/c_fcntl_begin_atomic_write.html#sqlitefcntlhasmoved).
