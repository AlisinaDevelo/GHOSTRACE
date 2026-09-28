---
id: 0164
title: Offer opt-in login-keychain key custody for unsigned builds
status: backlog
agent: maintainer
model: human
release: M4
depends_on: [0054, 0055]
change: null
workstream: storage
type: feature
priority: p1
risks: [privacy, security]
platform: macos
---

## Goal
Let an unsigned or ad-hoc-signed build keep its journal key in the user's login keychain, explicitly and with its weaker guarantees stated, while the data-protection keychain stays the default for signed builds.

## Acceptance criteria
- [ ] The login-keychain backend is selected only by an explicit flag or configuration and is reported as such by status output.
- [ ] The key item is non-synchronizable, created only by an explicit provisioning step, and never read or created implicitly.
- [ ] Documentation states the weaker binding (the item's access list follows the binary's signature, so rebuilds prompt) and the unchanged encryption format.

## Context
Developer ID signing costs money the project does not have yet; without it the data-protection keychain refuses the binary, which blocks every durable live journal.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.
