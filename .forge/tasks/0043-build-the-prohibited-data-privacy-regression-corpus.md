---
id: 0043
title: Build the prohibited-data privacy regression corpus
status: ready
agent: test-engineer
model: human
release: M0
parent: 0005
depends_on: [0004, 0006]
change: null
workstream: privacy
type: test
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Turn the data-minimization contract into an adversarial corpus that fails whenever forbidden content crosses a public boundary.

## Acceptance criteria
- [ ] Fixtures cover credentials, environment variables, command arguments, window titles, page content, clipboard text, and private-browser markers.
- [ ] Every ingest, error, diagnostic, explanation, and export surface is exercised against the corpus.
- [ ] CI reports only case identifiers and never echoes the injected sentinel values.

## Context
The privacy model is credible only when prohibited fields are tested at every serialization and error boundary.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
