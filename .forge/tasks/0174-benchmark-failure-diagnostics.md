---
id: 0174
title: Report why the native filesystem benchmark test fails under load
status: done
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
- [x] Every assertion in the native benchmark test names the scenario, run, and measured value against its bound.
- [x] The test's 3 s drain deadline and 30 s scenario bound are derived from the corpus resource budget, not fixed in the test.
- [x] The test passes ten consecutive runs alongside the full suite on the reference device, or the failure is recorded with its reason.

## Context
On 2026-09-28 the test failed once while the whole suite ran in parallel and passed alone and in two full reruns; the output did not say what failed.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.

Implemented and reviewed in PR #410, merged to protected main as
`9aa1cdee25ebbe04bb6628c61bd3ae35597350ef`. This task uses the recorded-failure
alternative, not a ten-green-run claim: the reviewed candidate's second event-storm
run took 50,793 ms against 30,000 ms. The exact merged-tree rerun passed, showing
the timing-sensitive result rather than resolving it by changing the bound.
Retained evidence and limitations:
[docs/evidence/0174-benchmark-failure-diagnostics.md](../../docs/evidence/0174-benchmark-failure-diagnostics.md).
