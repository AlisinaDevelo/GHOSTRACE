# ADR 0004: Own the FSEvents stream on one run-loop thread

## Status

Accepted for the lifecycle-adapter boundary. This ADR does not authorize ambient
capture or close the selected-root, consent, normalization, cursor, or persistence
gates.

## Context

`FSEventStreamCreate` crosses from Rust into CoreServices with a C callback,
borrowed callback arrays, and an explicit schedule/start/stop/invalidate/release
lifecycle. A callback context that outlives or moves independently of the native
stream can become a use-after-free. A callback panic must not cross the C ABI.

## Decision

`FseventsStream` is an owner-thread RAII wrapper:

1. Creation validates a non-empty, bounded path list and builds the Core Foundation
   path array with the type callbacks.
2. The callback context is one boxed Rust value whose pointer is copied into the
   native context with no retain/release callbacks. The wrapper remains `!Send` and
   `!Sync` and records the creating thread.
3. The owner explicitly schedules the stream on its current `CFRunLoop` in the
   default mode, then starts it. Flush, stop, and restart are available only in
   valid states. A second schedule, start, or stop is rejected without a second
   native call.
4. Callback pointers and event arrays are null-checked and bounded before copying
   paths. The user callback is wrapped in `catch_unwind`; a panic increments a
   bounded health counter and never unwinds through CoreServices. Callback modes
   that change the path representation (CFType, extended data, full history, or
   document IDs) are rejected before creation.
5. Shutdown stops a running stream, invalidates a scheduled stream, releases the
   native reference exactly once, and only then reclaims the boxed callback state.
   Invalidation is idempotent; an unscheduled partial construction is released
   without an invalidation call because CoreServices requires scheduling before
   invalidation.

## Alternatives rejected

- A detached callback thread would make run-loop ownership and shutdown ordering
  implicit and would require a queue/backpressure contract that belongs to the
  collector task.
- `FSEventStreamSetDispatchQueue` would move the platform contract to libdispatch
  before the project has decided its collector executor policy.
- A broad filesystem watcher would not provide FSEvents history/cursor semantics
  and would blur the source boundary.

## Consequences and limits

The adapter is safe to compose with a future collector, but it does not canonicalize
roots, enforce consent, reject symlink escapes, normalize flags, persist cursors,
or guarantee event completeness. Those capabilities require separate acceptance
evidence. The macOS integration and sanitizer lanes must remain device-specific;
unsupported OS/architecture rows are explicit no-go results.
