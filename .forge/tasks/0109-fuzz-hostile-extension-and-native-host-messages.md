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

Seven authentic corpus-guided libFuzzer targets under `fuzz/` call production
frame/schema, origin/transport, pairing, sequence and policy APIs using synthetic
seeds. They do not launch browsers, open listeners or write the journal. Native
host ingress is integrated; reconnect recovery merged in PR #430
(`f3fe7c1a7fa9c9f8b6124cdfca38532ecc5cb5ed`).

PR #433 merged the bounded runner and continuous CI at
`1f78a3aa92eaee63fdd7012bf69d0b978499ccc6`. A clean-source Apple M5 campaign on
2026-10-11 ran all seven targets for 60 seconds each: 8,417,188 executions,
peak reported RSS 139 MB, all passed, no failure artifacts, unchanged inputs.
Receipt `R-FUZZ-60-M5-1f78a3a/receipt.json` has SHA-256
`1612b3bb0fd0dc62ed7bb002c7f8775964bf959c172e09923b6728535d5372c4`.
PR #431 (`b2f86a20bd7ee3b8e02dc98237a2218a1e1dd8f8`) aligned action pins and
suppressed generated Python bytecode; clean hosted artifact 11689810518 records
seven passing targets at source `e886e997145446b5b94758875d3912da4d847b91`.

The [native completion evidence](../../docs/evidence/0030-native-host-completion.md)
records these distinct observations and their hashes, plus the clean combined
merged-source `ee4216c` reproduction on 2026-10-13: 102 distinct native checks
and seven full fuzz targets, 18,025,804 executions, peak reported RSS 168 MB,
no failure artifacts and unchanged inputs. Earlier M5 measurements remain bound
to `1f78a3a`.
The [historical fuzz receipt](../../docs/evidence/0109-native-protocol-fuzz-candidate.md)
preserves the earlier candidate, M1, interrupted-attempt and ASAN OOM history by
source. A build, seed replay or one-second campaign is not the full campaign.

Task 0109 remains in-progress and open for reviewed minimized regressions,
accepted/rejected production host/service CPU, memory, message and queue coverage,
and secret-bearing diagnostic/fault-oracle preservation. The runner retains
hashed failures and does not automatically minimize them. Safari normalization
is a pure envelope fixture; real packaging, permissions and runtime parity remain
0034/0110 gates.
