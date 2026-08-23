---
id: 0020
title: Define and ship JSONL export v1 with manifest
status: backlog
agent: maintainer
model: human
release: M3
depends_on: [0018, 0019]
change: null
---

## Goal
Ship a documented, streaming, versioned export that carries enough context to interpret journal records outside the application.

## Acceptance criteria
- [ ] The manifest includes version, policy, coverage, collector status, and gaps.
- [ ] Export streams safely.
- [ ] Existing destinations are not overwritten without explicit confirmation.

## Context
JSONL is the first portable compatibility boundary. Exports are user initiated and must warn when sensitive plaintext may leave encrypted storage.

## Notes
The bounded fixture exporter atomically writes a versioned manifest, policy ID, coverage, collector status, and gap records, and refuses overwrite without --force. Streaming large-journal behavior and dependent query/explain gates remain open.
