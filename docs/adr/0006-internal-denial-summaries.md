# ADR 0006: Summarize internal storage denials after observation stops

## Status

Accepted for the existing selected-root internal-artifact boundary. This does not
authorize another source, permission, field, or background collector.

## Context

A selected folder may contain the journal home and registered output artifacts.
Their notifications are denied before path hashing or filesystem-event admission.
Persisting an internal-denial summary during observation nevertheless changes the
journal's event set. A registry or temporary-file update can therefore invalidate
an explicitly approved export snapshot, and the summary's own journal write can
produce more internal notifications.

A native file-backed regression demonstrated that an excluded internal write
changed the export snapshot solely through its policy-blocked summary. Export
confirmation correctly refused that changed snapshot; the refusal must remain.

## Decision

- Keep internal denials as bounded counters while the native stream is running.
  `internal_path_denials` remains available in collector status.
- After stopping the stream and draining already observed callbacks, persist one
  path-free `internal_storage_path` aggregate, then the stop lifecycle event.
  Revocation also stops the stream before persisting its already-counted aggregate;
  pending observations remain discarded as before.
- Keep outside-scope denial summaries on the existing per-drain path. This change
  does not defer genuine policy denials or admit any previously rejected path.
- Report a live watch's final counters after the final drain. Announce `Watching`
  only after native startup succeeds, not before the collector is started.
- Preserve export plan/snapshot confirmation, authentication, private temporary
  files, validation, atomic publication, and changed-snapshot refusal.

## Consequences and limits

The aggregate contains a reason and count, never a path, digest, or filesystem
payload. It cannot cause journal recursion while its stream is observing. A crash
or failed terminal admission can lose the pending aggregate; it is not a durable
live denial count or a completeness guarantee. Existing bounded-writer failure
and dropped-event reporting remain applicable.

The regression checks actual native internal delivery, event-snapshot stability,
and a single aggregate matching the counter on both stop and revocation. CLI
acceptance waits for actual readiness, checks the completed journal for forbidden
filesystem rows, and reaps the watch before deleting its throwaway key/home even
when an assertion unwinds. Real event mutations still invalidate export previews.
No incremental-authentication or large-journal performance claim is made.
