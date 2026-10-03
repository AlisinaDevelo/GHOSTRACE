---
id: 0109
title: Fuzz hostile extension and native-host messages
status: in-progress
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

Candidate corpus-guided package is present under `fuzz/`, with one authentic
libFuzzer target for each required boundary and checked-in seeds. The targets
call the production frame decoder, schema parser, origin/transport validators,
pairing state, protocol sequence state, and policy conversion APIs; they do not
open sockets, invoke browsers, or write the journal. `fuzz/README.md` defines
bounded campaign flags and receipt requirements.

This note is not completion evidence: no campaign has been run in the current
resource-constrained handoff, and no minimized-crash, hang, memory, CPU, or
diagnostic receipt exists yet. The Safari target remains a pure envelope
profile; real Safari extension packaging, permissions, and runtime parity are
separate gates in tasks 0034/0110.
