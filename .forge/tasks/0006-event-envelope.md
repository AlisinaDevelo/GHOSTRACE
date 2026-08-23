---
id: 0006
title: Define the versioned canonical event envelope
status: done
agent: maintainer
model: human
release: M1
depends_on: [0003]
change: null
---

## Goal
Define the stable event contract shared by fixture ingestion, live collectors, storage, explanation, and export.

## Acceptance criteria
- [x] The typed source, kind, payload, and provenance model includes schema version and event ID.
- [x] The model includes source cursor, timestamps, policy ID, and confidence.
- [x] Golden serialization fixtures pass.

## Context
The envelope must preserve source facts while clearly distinguishing direct, inferred, and unknown confidence.

## Notes
Event envelope v1, JSON Schema, causal-chain fixture, and checked-in golden serialization are covered by the 0.0.1 integration suite.
