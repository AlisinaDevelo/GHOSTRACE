---
id: 0011
title: Build fixture replay and crash-injection harness
status: done
agent: maintainer
model: human
release: M1
depends_on: [0006, 0010, 0062]
change: pr-233-252805ee381316935ac8ef38c12cb6be6abb1636
workstream: storage
type: test
priority: p0
risks: [security]
platform: any
---

## Goal
Build repeatable evidence that normalized events, acknowledgements, and loss reporting remain correct across replay and process failure.

## Acceptance criteria
- [x] Deterministic multi-source fixtures replay identically.
- [x] Injected crashes create no false acknowledgements.
- [x] Any loss becomes an explicit gap.

## Context
The corpus should exercise filesystem, frontmost-application, shell, Git, and browser-shaped inputs without enabling live collectors.

## Notes
The checked-in multi-source causal fixture replays deterministically and carries an explicit gap. The parent-level replay harness now compares two deterministic journals byte-for-byte, aborts a writer child before commit, checks for no acknowledgement marker or durable event/cursor, and retries after restart. The verified scope remains fixture-only; live collector recovery is separate work.
