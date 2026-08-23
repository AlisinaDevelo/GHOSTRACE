# ADR 0002: Start with FSEvents; defer Endpoint Security

- **Status:** Accepted
- **Date:** 2026-08-23
- **Scope:** macOS source selection

## Context

GHOSTRACE needs a first live source that can demonstrate selected-root scope,
minimized fields, restart behavior, and explicit gaps. macOS offers filesystem
change notifications through FSEvents and richer security-event observation through
Endpoint Security. The latter is entitlement-gated and introduces a broader
permission, attribution, and operational boundary.

The planned development floor for the first live source is macOS 15.0 (Sequoia) on
Apple silicon and Intel. The release gate must revalidate that floor and may raise
it if the supported APIs or security controls require it.

## Decision

When live capture is enabled, implement selected-root FSEvents metadata first.
FSEvents output must carry source flags and cursor state, and must disclose that it
can coalesce, delay, reorder, omit, or fail to attribute changes. It must never be
presented as a complete causal trace.

The initial live-source development floor is macOS 15.0 (Sequoia), targeting both
Apple silicon and Intel. The release gate must test both architectures against the
then-supported macOS matrix and may raise this floor rather than claim an untested
configuration. The fixture core remains separately testable on Linux.

The FSEvents baseline requires no root access, privileged helper, Accessibility, or
Automation permission. It does not claim Full Disk Access; any later source that
needs a TCC grant must document and test that grant before its code is enabled.

Endpoint Security remains optional and deferred. It may be considered only through a
separate decision that documents entitlements, minimum permissions, event
allowlisting, private-context behavior, resource bounds, failure gaps, and
user-facing consent. It is not a hidden fallback or an automatic upgrade.

## Alternatives considered

1. **Endpoint Security first:** rejected because attribution value does not justify
   making the initial product depend on a broader entitlement and trust boundary.
2. **Use both by default:** rejected because duplicate or conflicting sources would
   increase collection and explanation complexity before the coverage model exists.
3. **Filesystem polling:** rejected because it is less efficient, can miss short-lived
   changes, and does not improve causal attribution.

## Consequences

Positive:

- The first live path has a narrow selected-root scope and no content access.
- Gap and recovery semantics can be tested before process attribution is introduced.
- The product does not require Endpoint Security entitlements for its baseline.

Costs:

- FSEvents cannot reliably attribute a change to a process.
- Coalescing and historical loss can leave unknown intervals.
- Some explanations remain weaker than they would be with an optional,
  entitlement-gated source.
