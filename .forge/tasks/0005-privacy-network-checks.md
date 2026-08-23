---
id: 0005
title: Add privacy regression and network-surface checks
status: ready
agent: maintainer
model: human
release: M0
depends_on: [0003, 0004]
change: null
---

## Goal
Make the privacy contract executable by testing prohibited fields and the absence of unexpected network behavior.

## Acceptance criteria
- [ ] Regression fixtures prove prohibited fields are absent.
- [ ] Dependency and network policy is documented.
- [ ] Linux CI runs fixture tests inside an enforced network-denied environment; if runner restrictions block that mechanism, a checked-in decision record captures the failed mechanism and the equivalent enforced offline test.

## Context
Checks must cover sensitive command, browser, filesystem, and application fields before live collection is enabled.

## Notes
The headstart includes strict unknown-field rejection, browser URL sanitization, private-context refusal, ciphertext-at-rest checks, and documented dependency policy. A network-denied CI execution and broader prohibited-field corpus are still required.
