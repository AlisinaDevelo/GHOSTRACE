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

The SQLite payload column stores the authenticated `GRCE` ciphertext envelope rather
than a bare nonce. Its public metadata is the envelope schema version, algorithm, and
key generation; the nonce and ciphertext are bounded bytes, and the key itself is
resolved only through the active local provider. Legacy nonce-plus-ciphertext rows can
still be read while a verified rotation migrates them. Rotation checkpoints and
destruction receipts are separate key-free contracts and are never event payloads.

For a future FSEvents source, the raw callback flag word is normalized by the
`fsevents-normalized-v1` contract before it can become a filesystem event. The
contract maps all documented Apple bits, retains the raw `u32` and unknown-bit
remainder, and records a status of observed, rescan-required, boundary,
unsupported, or contradictory. Unknown bits and loss boundaries lower completeness;
they are never silently treated as ordinary file changes. The normalized evidence
record is path-free, so selected-root canonicalization and exclusion policy still
run before persistence.

The published JSON Schema describes the canonical normalized serialization emitted
by Rust, so nullable/defaulted fields are present even when their value is `null` or
`false`. Rust semantic validation remains mandatory for cross-field timestamp
ordering, UTF-8 byte limits, and other invariants JSON Schema cannot express
portably. A structural or semantic breaking change creates a new schema version and migration
rule; consumers must not guess at unknown fields.

## Ingestion origin capabilities

An event's `source` says what the observation describes; it does not grant a caller
permission to assert that provenance. The journal therefore requires an
`IngestionOrigin` capability for every `ingest` and `ingest_batch` call. The capability
owns the provenance version and collector-instance namespace instead of accepting those
values as caller-supplied strings.

The four origin paths are deliberately separate:

| Origin | Construction boundary | Allowed event classes |
| --- | --- | --- |
| Fixture | Public `IngestionOrigin::fixture()` path | All normalized fixture event kinds, including lifecycle and gap/status records |
| Live | In-crate collector adapter | All live normalized event kinds, with a `live-` instance and `live-v1` provenance |
| Import | In-crate import adapter | Normalized observations and status records; imported lifecycle assertions are refused |
| Repair | In-crate recovery adapter | Gap, policy-blocked-summary, and source-error records only |

Live, import, and repair capabilities carry a private token. Events constructed in
memory retain that token outside the wire format, so deserializing a fixture never
creates a live capability binding. The fixture path accepts deserialized envelopes only
when their provenance remains `fixture-v1` and their collector instance remains in the
`fixture-` namespace. A caller cannot relabel a fixture as a live collector through the
generic journal API.

## Semantic identifier and digest contract

Event v1 does not accept arbitrary metadata in identifier-shaped fields. The
serialized Rust boundary remains a string for compatibility, but each string has the
following frozen semantic type and constructor validation. Values are ASCII-only so
there is no Unicode normalization or alternate-encoding ambiguity. Rejections never
echo the candidate value.

| Fields | Semantic type | Canonical encoding and bound | Sensitivity classification |
| --- | --- | --- | --- |
| `event_id`, `parent_event_id` | UUID | RFC 4122 hyphenated UUID; non-nil; `parent_event_id` is nullable | Random journal pseudonym; linkable metadata |
| `root_id`, `repository_id`, `session_id`, `shell_kind`, `browser`, `bookmark_id`, `folder_id`, `collector_instance`, `instance_label`, `provenance_version`, `policy_profile_id` | Opaque identifier | Lowercase ASCII token `[a-z0-9][a-z0-9._-]*[a-z0-9]`, no `..`, 1–128 bytes; nullable only where the schema says so | Potentially identifying or policy metadata; retained only as bounded labels |
| `app_id`, `previous_app_id` | Application identifier | Lowercase reverse-DNS labels (`com.example.app`), each label 1–63 bytes, total 1–255 bytes | Potentially identifying software metadata |
| `branch` | Git branch name | ASCII Git ref label, 1–255 bytes; `/` is allowed for namespaces, but empty components, traversal (`..`), `//`, `@{`, ref metacharacters, and `.lock` suffixes are rejected | Repository context; may reveal project or workflow names |
| `head_oid` | Git object ID | Lowercase hexadecimal, exactly 40 or 64 bytes | Public-derived repository identity; linkable |
| `path_digest`, `snapshot_digest` | SHA-256 digest | Tagged lowercase form `sha256:` plus exactly 64 lowercase hexadecimal bytes (71 bytes total) | Derived, potentially dictionary-linkable; never a plaintext path or snapshot |
| `source_cursor`, `from_cursor`, `to_cursor` | Source cursor token | Lowercase ASCII token, 1–256 bytes, no `..`; nullable where shown | Source position and coverage metadata; may reveal collection state |
| `reason_code` | Reason code | Lower snake case `[a-z][a-z0-9_]*`, 1–128 bytes | Bounded operational/policy category; must not contain the denied value |
| `url` | Sanitized URL | `http` or `https`, host required, at most 8192 bytes; userinfo, query, and fragment are removed | Potentially identifying origin metadata; private contexts are refused |

The JSON Schema uses the same patterns and bounds as Rust validation. `EventEnvelope::new`
and envelope deserialization are the acceptance constructors; payloads are accepted only
after the envelope invokes the same semantic checks. The M1 semantic wrappers now expose
distinct fallible constructors for these retained values. Serde uses those same
constructors, so a value cannot enter a payload through a deserialization shortcut, and
the wrappers serialize back to the unchanged string wire encoding.

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
