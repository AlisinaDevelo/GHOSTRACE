---
id: 0030
title: Implement Native Messaging host and explicit pairing
status: backlog
agent: maintainer
model: human
release: M5
depends_on: [0008, 0010, 0029]
change: null
---

## Goal
Create a bounded, authenticated local bridge between an explicitly paired browser extension and the journal ingestion path.

## Acceptance criteria
- [ ] Only paired extension IDs are accepted.
- [ ] Messages are bounded and schema-validated.
- [ ] Malformed input cannot execute commands or escape journal paths.
- [ ] Pairing can be revoked.

## Context
The host must expose no generic command execution or filesystem capability. Pairing credentials and journal keys are separate security assets.

## Notes
No implementation notes yet.
