---
id: 0174
title: Report why the native filesystem benchmark test fails under load
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0076]
change: null
workstream: filesystem
type: test
priority: p2
risks: []
platform: macos
---

## Goal
Make a failure of the native filesystem benchmark test say which scenario and bound failed, so a failure under load can be fixed rather than retried.

## Acceptance criteria
- [ ] Every assertion in the native benchmark test names the scenario, run, and measured value against its bound.
- [ ] The test's 3 s drain deadline and 30 s scenario bound are derived from the corpus resource budget, not fixed in the test.
- [ ] The test passes ten consecutive runs alongside the full suite on the reference device, or the failure is recorded with its reason.

## Context
On 2026-09-28 the test failed once while the whole suite ran in parallel and passed alone and in two full reruns; the output did not say what failed.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
