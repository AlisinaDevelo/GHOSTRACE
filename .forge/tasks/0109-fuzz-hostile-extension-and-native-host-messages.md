---
id: 0109
title: Fuzz hostile extension and native-host messages
status: backlog
agent: test-engineer
model: human
release: M5
parent: 0033
depends_on: [0103, 0104]
change: null
workstream: browser
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Continuously test framing and semantic boundaries with malformed, adversarial, state-confused, and resource-exhausting browser messages.

## Acceptance criteria
- [ ] Corpus-guided fuzz targets cover frame decoder, schema parser, origin validation, pairing state, sequence handling, and policy conversion.
- [ ] Memory, CPU, message, queue, and diagnostic output stay within explicit limits for accepted and rejected inputs.
- [ ] Crashes, hangs, unexpected acceptance, and secret-bearing diagnostics preserve minimized regressions in CI.

## Context
The native host must treat every extension message as untrusted even after pairing.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
