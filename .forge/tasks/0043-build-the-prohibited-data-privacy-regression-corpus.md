---
id: 0043
title: Build the prohibited-data privacy regression corpus
status: done
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
- [x] Fixtures cover credentials, environment variables, command arguments, window titles, page content, clipboard text, and private-browser markers.
- [x] Every ingest, error, diagnostic, explanation, and export surface is exercised against the corpus.
- [x] CI reports only case identifiers and never echoes the injected sentinel values.

## Context
The privacy model is credible only when prohibited fields are tested at every serialization and error boundary.

## Notes
Implemented and verified under evidence artifacts `GHOSTRACE-0043-PRIVACY-CORPUS-V1`,
`GHOSTRACE-0043-PREMERGE-AFC0E3B`, `GHOSTRACE-0043-MERGED-AC7B4CC`,
`GHOSTRACE-0043-LOCAL-PIPELINE-98420EE`, `GHOSTRACE-0043-LOCAL-AUDIT`,
`GHOSTRACE-0043-LOCAL-DENY`, and `GHOSTRACE-0043-LOCAL-NETWORK`.

Pre-merge implementation commit: `afc0e3bee2517cb18426df3420ef0229cad81624`.
PR #167 was reviewed locally, passed required GitHub checks, and merged to
protected `main` at `ac7b4cc878ffadbcf43ccbaec15c99b7588b4226` on 2026-08-24.
The same reproduction was rerun on that merged SHA on a MacBookPro17,1 (Apple
M1, 8 GB), macOS 26.6.2 (25G83), arm64, Darwin 25.6.0, with rustc/cargo
1.88.0; debug and release test suites each passed 19 tests and the focused
corpus test passed. The merged-run log digest is
`f3886a325650b3be32670875802509f45697259878ff8c9bc51c2154f210a2c3`.
The retained report is `docs/evidence/0043-privacy-regression.md`; local
pre-merge and merged logs were scanned for the sentinel prefix and were clean.
The expanded offline local pipeline passed locked metadata/check/build, debug
and release suites, five repeated privacy runs, doctests, Clippy, formatting,
actionlint, roadmap checks, CLI demo/export/schema/capture checks, RustSec
audit, cargo-deny policy, and network-surface scans. Its log digests are
`cca66853751602af4d1db0d14d85bf5f07e1ebec7a75542abe4b3b3c087f6b63`,
`bdbcfb9e01bfd07f6b14cb986421667eec10a2da27e230821584ec21631e8403`,
`2f1e8510d523fa397202e9f6938b1f7da99ed8755e954710df9d412adf239025`, and
`df3e48b3dd76e3e4944d49ce121ba320fe85b74b717cadd934be42e6c82f9030`.

The corpus is synthetic and fixture-only; it does not exercise live capture,
macOS permissions, or hardware-dependent paths. The local audit/policy tools
passed in task-scoped temporary installations. Completion requires the
acceptance evidence above; issue closure must link these artifacts, logs,
digests, limitations, and the merged SHA.
