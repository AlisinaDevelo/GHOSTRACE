---
id: 0047
title: Define program outcomes and the release evidence register
status: done
agent: tech-lead
model: human
release: M0
depends_on: [0001, 0004]
change: null
workstream: foundation
type: docs
priority: p0
risks: [privacy, security]
platform: any
---

## Goal
Give every milestone measurable product, privacy, correctness, performance, and operability outcomes with durable evidence references.

## Acceptance criteria
- [x] Each milestone has quantified or binary exit measures and names the artifact that proves each measure.
- [x] The register distinguishes planned, observed, inferred, and unavailable evidence.
- [x] A release cannot close a gate when required evidence is missing, stale, or scoped to a narrower surface.

## Context
The program needs a durable definition of done rather than completion by issue count or intent.

## Notes
Evidence: `docs/evidence/0047-release-evidence-register.md`.
Implementation merged by PR #177 at `ae90aa28425a40753ea385f710545c5df5ab2582`; the
exact merged-main checker, negative gate, and local pipeline results are recorded
in the evidence report.
