---
id: 0030
title: Implement Native Messaging host and explicit pairing
status: review
agent: maintainer
model: human
release: M5
depends_on: [0008, 0010, 0029, 0102, 0103, 0104]
change: null
workstream: browser
type: feature
priority: p1
risks: [security]
platform: any
---

## Goal
Create a bounded, authenticated local bridge between an explicitly paired browser extension and the journal ingestion path.

## Acceptance criteria
- [ ] Only paired extension IDs are accepted.
- [ ] Messages are bounded and schema-validated.
- [ ] Malformed input cannot execute commands or escape journal paths.
- [ ] Pairing can be revoked.

## Context
The host must expose no generic command execution or filesystem capability. Pairing credentials and journal keys are separate security assets.

## Notes
Local candidate on `feature/native-host-journal-bridge`; review, publication, and
merged-main reproduction remain pending. Checkboxes remain unchecked until the
repository completion evidence contract is met.

Acceptance tests:
- Paired IDs: `unpaired_hello_and_unauthenticated_navigation_never_reach_the_sink`,
  `caller_origin_must_match_the_hello_extension_before_pairing`, and
  `service_refuses_unpaired_tampered_and_revoked_proofs_before_journal_ingest`.
- Bounds/schema: `oversized_empty_truncated_and_invalid_frames_fail_closed`,
  `fields_and_strings_are_bounded_before_typed_deserialization`, and
  `malformed_commands_paths_and_oversized_messages_have_no_sink_or_file_effect`.
- Commands/paths: the malformed-command/path test above plus
  `service_request_rejects_path_retention_instead_of_silently_dropping_it` and
  `accepted_navigation_is_projected_through_the_journal_boundary`.
- Revocation: `revoking_an_active_pairing_stops_the_next_authenticated_frame`,
  `revocation_persists_and_tampered_store_is_refused`,
  `revocation_waits_for_an_inflight_admitted_sink_before_returning`, and
  `timed_out_service_write_retains_pairing_lease_until_worker_finishes`.

Only `CanonicalNavigation` under `OriginOnly` reaches the service. The service
independently checks a domain-separated pairing MAC, bound to its instance,
request ID, canonical admission, and stable delivery tuple. Its writer retains
the pairing lease through completion even when the requesting host times out.
The host has no journal key, command execution, filesystem-message capability,
or network client. See `docs/ARCHITECTURE.md` for the trust boundaries and
remaining browser/service integration scope.
