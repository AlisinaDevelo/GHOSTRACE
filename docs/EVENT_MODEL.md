# Event model

This document describes the versioned event contract that connects fixtures, future
source adapters, storage, explanations, and exports. It is a contract for bounded
observations, not a claim that an observation is a complete causal record.

## Envelope

An event has these conceptual fields:

| Field | Meaning | Invariant |
| --- | --- | --- |
| schema version | Version of the serialized envelope | Unknown versions fail closed |
| event ID | Stable identifier for one accepted observation | Unique within the journal and referenced by explanations |
| source | Adapter or fixture origin | Must be known and versioned |
| kind | Normalized observation kind | Enumerated; unknown kinds are not silently reinterpreted |
| observed time | Source or ingestion timestamp with its basis | Never presented as a causal timestamp without qualification |
| payload | Minimum normalized facts allowed by policy | Bounded; prohibited content is absent |
| provenance | Source cursor, policy ID and version, source flags, and normalization status | Preserves what the source reported and what policy did |
| evidence | direct, contextual, inferred, or unknown | A claim cannot be stronger than its supporting observations |
| gap/status | Missing coverage, denial, drop, restart, or source limitation | First-class and visible to explain/export |

The published JSON Schema describes the canonical normalized serialization emitted
by Rust, so nullable/defaulted fields are present even when their value is `null` or
`false`. Rust semantic validation remains mandatory for cross-field timestamp
ordering, UTF-8 byte limits, and other invariants JSON Schema cannot express
portably. A structural or semantic breaking change creates a new schema version and migration
rule; consumers must not guess at unknown fields.

## Evidence levels

- **Direct:** the source reported the fact itself, such as a fixture event containing
  a normalized change kind.
- **Contextual:** a fact that gives bounded context, such as a selected policy,
  source status, or time window, without asserting causality.
- **Inferred:** a deterministic relationship derived from accepted observations,
  such as ordering or a supported adjacency. The explanation must cite its inputs
  and state the inference rule.
- **Unknown:** the source or policy cannot establish the fact. Unknown is preferable
  to an unsupported inference.

Evidence level describes support, not confidence in a person or an intention.

## Gaps

A gap records that the journal cannot account for an interval, source result, or
policy outcome. Examples include:

- a fixture or source line rejected as malformed;
- a source cursor that is invalid, wrapped, or no longer recoverable;
- queue pressure or a forced drop;
- FSEvents coalescing, delay, omission, or non-attribution;
- a restart interval that was not covered;
- a private or out-of-scope observation intentionally denied.

Gaps carry enough bounded status to explain the limitation without retaining the
blocked sensitive value. Explanations and exports must include relevant gaps rather
than treating them as empty space.

## Causal links

A causal link is an explanation edge between observations, not a legal or scientific
proof. Each edge must identify:

1. the source event IDs;
2. the rule or ordering relation used;
3. each supporting evidence level;
4. relevant policy and coverage context;
5. gaps that could weaken the interpretation.

No edge may be created from a missing event, a private context, an unbounded source,
or a time ordering alone when the source cannot establish the relationship.

## Serialization and compatibility

- JSONL records are self-describing by schema version and event ID.
- Replaying the same valid fixture with the same policy produces stable normalized
  records and explanation output.
- Export must preserve evidence labels, provenance, and gaps.
- Unknown schema versions, malformed UUIDs, invalid timestamps, and prohibited
  payload fields fail closed.
- Event IDs are opaque identifiers; consumers must not derive meaning from their
  textual form.

## Privacy rules

The event model does not include keystrokes, microphone or screen data, clipboard
values, window titles, page contents, file contents, credentials, environment
variables, command arguments, standard input, or standard output. A future adapter
must justify every new field in the privacy and threat documents before adding it.
