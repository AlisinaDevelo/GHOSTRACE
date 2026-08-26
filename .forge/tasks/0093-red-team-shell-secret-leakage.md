---
id: 0093
title: Red-team shell secret leakage
status: done
agent: security-auditor
model: human
release: M4
parent: 0024
depends_on: [0091]
change: pr-321
workstream: shell-git
type: test
priority: p1
risks: [privacy, security]
platform: any
---

## Goal
Prove that common credential, environment, prompt, path, process-title, diagnostic, and crash-report channels do not enter retained shell evidence.

## Acceptance criteria
- [x] The corpus covers tokens in arguments, environment, stdin, stdout, stderr, executable names, working paths, failure messages, prompts, process titles, diagnostics, crash-report context, and command text.
- [x] Journal, logs, errors, exports, panic output, and process inspection are checked for unique sentinels.
- [x] Any unavoidable operating-system exposure is documented separately from GHOSTRACE retention claims.

## Context
The wrapper boundary is valuable only if its negative data contract survives failure paths.

## Notes
Implemented in PR #321 and squash-merged to protected `main` at
`489563e8106a66f206f40ba5fa0ccd0c7ae7cef5`. The deterministic corpus and six-test
red-team suite reject or omit every application-retained sentinel. macOS process
inspection is verified as external exposure and explicitly not retained; non-macOS
runners record a no-go where the OS inspector is unavailable. See
[`docs/evidence/0093-shell-secret-leakage.md`](../../docs/evidence/0093-shell-secret-leakage.md)
for device receipts and command-level evidence.
