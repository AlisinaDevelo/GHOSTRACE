---
id: 0135
title: Specify an offline OpenTelemetry import profile
status: backlog
agent: observability-specialist
model: human
release: M8
parent: 0132
depends_on: [0125, 0133]
change: null
workstream: interoperability
type: spike
priority: p2
risks: [privacy, security]
platform: any
---

## Goal
Evaluate importing explicitly selected local OTLP traces or logs as contextual evidence without adding an exporter, listener, agent, or default network dependency.

## Acceptance criteria
- [ ] The profile allowlists resource, scope, trace, span, event, status, and attribute fields and rejects arbitrary bodies and sensitive attributes.
- [ ] Trace and span relationships remain source claims and never become proof of user intent or filesystem causality.
- [ ] A go or no-go ADR includes privacy leakage, size, compatibility, malicious input, and usefulness measurements.

## Context
OpenTelemetry can provide explicit application context, but GHOSTRACE will not become a network telemetry pipeline.

## Notes
Planned in the 2026–2031 GHOSTRACE program. Completion requires the acceptance evidence above; issue closure alone is not evidence.
