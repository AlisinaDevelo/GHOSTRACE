---
id: 0172
title: Keep login-keychain access stable across rebuilds without a Developer ID
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0164]
change: null
workstream: storage
type: feature
priority: p2
risks: [security]
platform: macos
---

## Goal
Stop macOS from asking for keychain approval after every rebuild of an unsigned GHOSTRACE binary, at no cost.

## Acceptance criteria
- [ ] A documented script creates a local self-signed code-signing identity in the login keychain and signs the built binary with a stable identifier.
- [ ] A rebuilt, re-signed binary reads the journal key without a new approval prompt on the reference device; an unsigned or differently signed binary still prompts.
- [ ] The identity's private key never leaves the login keychain, the script is idempotent, and removal is documented.

## Context
The login-keychain item's access list follows the binary's designated requirement. An ad-hoc signature changes on every build; a stable certificate keeps the requirement the same.

## Notes
This is not a substitute for Developer ID signing and notarization (0122) and does not enable the data-protection keychain.
Completion requires the acceptance evidence above; issue closure alone is not evidence.
