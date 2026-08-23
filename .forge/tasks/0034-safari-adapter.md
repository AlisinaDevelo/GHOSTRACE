---
id: 0034
title: Decide Safari WebExtension parity and ship only if viable
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0029, 0030, 0031, 0032, 0033, 0110]
change: null
workstream: browser
type: feature
priority: p2
risks: [privacy]
platform: macos
---

## Goal
Resolve Safari coverage with an evidence-backed go or no-go decision, shipping an adapter only if it preserves every contract already proven for Chromium.

## Acceptance criteria
- [ ] The parity gate maps Safari constraints to the same event schema, capture policy, pairing, redaction, private-mode, lifecycle, and deduplication contracts used for Chromium.
- [ ] A go decision ships a bounded adapter that passes those contracts and the browser threat corpus.
- [ ] A no-go decision publishes the failed requirements and evidence, ships no Safari adapter or fallback listener, and leaves the Chromium baseline unchanged.

## Context
Safari support is optional. If viable, it remains an adapter over the same local ingestion boundary; platform differences cannot be hidden behind a lowest-common-denominator claim.

## Notes
No implementation notes yet.
