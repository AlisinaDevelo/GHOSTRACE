---
id: 0117
title: Publish SBOM and SLSA build provenance
status: backlog
agent: supply-chain-specialist
model: human
release: M6
parent: 0038
depends_on: [0116]
change: null
workstream: release-scale
type: feature
priority: p0
risks: [security]
platform: any
---

## Goal
Bind every distributed artifact to source, build workflow, resolved dependencies, SBOM, builder identity, and verification instructions.

## Acceptance criteria
- [ ] The release publishes versioned SPDX or CycloneDX SBOMs for every artifact and includes Rust, native, and packaged components.
- [ ] SLSA provenance identifies artifact digests, source revision, builder, workflow, parameters, and dependencies according to the chosen level.
- [ ] An offline verifier rejects altered artifacts, provenance, SBOMs, source references, and unexpected builders.

## Context
SLSA 1.2 distinguishes provenance existence, authenticity, and hardened build guarantees; the project will state only the level it proves.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
