---
id: 0175
title: Make per-write authenticated-state verification incremental
status: done
agent: maintainer
model: human
release: M4
depends_on: [0088]
change: null
workstream: storage
type: feature
priority: p2
risks: [security]
platform: any
---

## Goal
Keep each journal write's authenticated-state check fast as the journal grows, without weakening tamper detection.

## Acceptance criteria
- [x] A write verifies and advances the anchor from the previous anchor and the rows it adds, not by recomputing digests over every stored event.
- [x] The full recomputation remains available and runs in `authenticated-check`, at startup after an unclean shutdown, and when another process's commit is detected.
- [x] Write-lock hold time stays within a documented bound at 100,000 events on the reference device, and two concurrent writers never time out under the default busy timeout.

## Context
Every write recomputes the canonical snapshot over all events while holding the write lock (since the lock fix in #402), so a writer's lock hold time grows with the journal and a second GHOSTRACE process can wait past the default 250 ms busy timeout. The live CLI waits up to 10 s as a stopgap.

## Notes
Local implementation and device acceptance passed on 2026-10-04. The change
preserves v1 interpretation and full tamper checks, adds a versioned appendable
commitment, and retains the single write guard.
Full external-commit revalidation must happen outside the reserved write lock;
the guard must reject a changed database/anchor before mutation. Cached state is
published only after commit and invalidated after rollback, rotation or deletion.

Acceptance includes a retained failing baseline, deterministic adversarial and
cross-process tests, 100,000-event device measurements with the unchanged default
busy timeout, independent review, local pipeline, merge and exact-main reproduction.
Completion requires all criteria above; issue closure alone is not evidence.

## Local acceptance evidence

- Reference device: Apple M1 MacBook Pro, 8 GB RAM, macOS 26.6.2 (25G83),
  arm64, Rust/Cargo 1.88.0, unoptimized all-features tests.
- 100,000-event seed: 110.320 s. Reopened first-write full preflight: 14.486 s.
  Final full authentication check: 9.286 s. Startup made one backing-key read;
  each of the 64 hot writes made one read (64 total).
- 64 steady-state single-event writes: median 3.696 ms, p95 6.152 ms,
  max 11.364 ms against the predeclared 200 ms bound. Whole-write wall time
  conservatively bounds the write-lock interval; no direct lock instrumentation.
- Two independent connections and two separate processes each completed
  32 writes at the unchanged 250 ms busy timeout; no timeout or refusal.
  Process pair elapsed 33.549 s, including external-commit verification/retries.
- Full recomputation remains in `authenticated-check` and before the first write
  of every reopened writer (including an unclean restart); foreign commits force
  a fresh full snapshot, fenced again under the retained `IMMEDIATE` guard.
- Preflight, anchor authentication, encryption, and refresh share one key scope
  per write. Exact-one-read tests cover memory inserts/batches and file-backed
  bootstrap, steady-state writes, reopened preflight, and batches. Keys are not
  cached between writes; rotation resolves each distinct generation once.
- Real pre-migration binary: schema 6 refused; valid eight-event v1 anchor
  promoted in 3.165 ms; tampered v1 anchor refused without promotion.
- Legacy promotion, retention/rotation, batches, larger operation histories,
  and other devices are outside the measured steady-state bound.
- The preceding 28.682 s startup result was close to the unchanged 30 s reader
  limit. Full verification remains a linear scan; the limit needs a follow-up.

Acceptance checkboxes remain open because the roadmap validator permits checked
criteria only on done tasks. Local evidence above meets the implementation and
device criteria; status remains in-progress until independent review, push/PR, protected-main
merge and exact-main reproduction. Local logs and PR notes are in the maintainer's
`ghostrace-evidence/handoff-403.md`; no hosted checks or merge are claimed.
