# ADR 0001: Local-only, minimized capture

- **Status:** Accepted
- **Date:** 2026-08-23
- **Scope:** Product and data boundary

## Context

GHOSTRACE is intended to help a person explain bounded local changes. Ambient
activity collection would make the product more invasive than the problem requires,
would encourage unsupported causal claims, and would make a silent network or
permission expansion difficult to detect.

## Decision

GHOSTRACE is local-only and deny-by-default. The initial implementation is
fixture-only. A future source must be explicitly enabled by a versioned policy and
must retain only bounded metadata within a selected scope. The baseline does not
use keylogging, microphone input, screen recording, clipboard capture, window
titles, page contents, file contents, private browsing by default, root, Full Disk
Access, Accessibility, Automation, telemetry, URL fetching, cloud sync, a network
client, or silent upload.

The product records what its sources establish and exposes direct, contextual,
inferred, and unknown evidence levels. Gaps are first-class. No output is described
as legal chain of custody.

## Alternatives considered

1. **Ambient activity recorder:** rejected because it violates minimization and
   creates a broad, difficult-to-audit trust boundary.
2. **Remote service with cloud history:** rejected because it adds network,
   account, upload, and retention risks that are not needed for local explanation.
3. **Capture everything, redact later:** rejected because a later redaction cannot
   undo exposure to the process, logs, swap, backups, or a compromised host.
4. **Fixture-only research tool forever:** rejected as the long-term product, but
   retained as the safe starting point while live-source gates are built.

## Consequences

Positive:

- The initial behavior is offline, testable, and inspectable.
- The data inventory and permission boundary remain small.
- Users can reason about when a record exists and where an export goes.
- Source limitations are part of every explanation.

Costs:

- GHOSTRACE cannot answer questions requiring unselected or prohibited data.
- Some useful correlations remain unknown because the source cannot establish them.
- Every future collector requires policy, failure, and privacy evidence before release.
