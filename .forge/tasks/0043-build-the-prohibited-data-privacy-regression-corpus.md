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
`GHOSTRACE-0043-PREMERGE-AFC0E3B`, and `GHOSTRACE-0043-MERGED-AC7B4CC`.

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

The corpus is synthetic and fixture-only; it does not exercise live capture,
macOS permissions, or hardware-dependent paths. `cargo-audit` and `cargo-deny`
were unavailable locally and remain covered by their required CI jobs, not by
substitution. Completion requires the acceptance evidence above; issue closure
must link these artifacts, logs, digests, limitations, and the merged SHA.
