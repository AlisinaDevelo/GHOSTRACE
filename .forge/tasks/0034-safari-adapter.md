---
id: 0034
title: Add Safari WebExtension adapter
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0029, 0030, 0031, 0032, 0033]
change: null
---

## Goal
Extend browser coverage to Safari without weakening the contracts already proven for Chromium.

## Acceptance criteria
- [ ] Safari uses the same event schema and capture policy.
- [ ] Redaction and private-mode guarantees match Chromium.
- [ ] Pairing and deduplication guarantees match Chromium.

## Context
Safari support remains an adapter over the same local ingestion boundary. Platform differences must be documented rather than hidden behind a lowest-common-denominator claim.

## Notes
No implementation notes yet.
