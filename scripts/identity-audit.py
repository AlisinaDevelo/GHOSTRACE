#!/usr/bin/env python3
"""Validate the GHOSTRACE identity and namespace decision record.

This checker is deliberately offline. It validates the retained observations and
ensures that an unavailable registry or zero-result search is never converted
into a trademark, domain, or package-availability claim.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import date
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "planning" / "identity-gate.json"
DATE_RE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}\Z")
URL_RE = re.compile(r"https://[^\s]+\Z")
PACKAGE_RE = re.compile(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*\Z")
REVERSE_DNS_RE = re.compile(r"[a-z][a-z0-9]*(?:\.[a-z][a-z0-9-]*)+\Z")
REQUIRED_COLLISIONS = {
    "vusec-ghostrace",
    "github-search",
    "crates-io",
    "homebrew",
    "npm",
    "pypi",
    "domains",
    "major-search",
}
ALLOWED_RESULT_STATES = {
    "blocked",
    "collision",
    "lookup_unavailable",
    "mixed",
    "no_exact_record_observed",
    "no_rdap_record_observed",
    "registered",
}


class IdentityError(ValueError):
    """The identity manifest is malformed or makes an unsafe claim."""


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise IdentityError(f"cannot read identity manifest: {path}") from exc
    if not isinstance(value, dict):
        raise IdentityError("identity manifest root must be an object")
    return value


def require_string(value: Any, field: str) -> str:
    if type(value) is not str or not value.strip():
        raise IdentityError(f"{field} must be a non-empty string")
    return value


def require_url(value: Any, field: str) -> str:
    value = require_string(value, field)
    if not URL_RE.fullmatch(value):
        raise IdentityError(f"{field} must be an HTTPS URL")
    return value


def require_bool(value: Any, field: str) -> bool:
    if type(value) is not bool:
        raise IdentityError(f"{field} must be boolean")
    return value


def require_nonnegative_int(value: Any, field: str) -> int:
    if type(value) is not int or value < 0:
        raise IdentityError(f"{field} must be a non-negative integer")
    return value


def validate_identifier(value: Any, field: str, pattern: re.Pattern[str]) -> str:
    value = require_string(value, field)
    if not pattern.fullmatch(value):
        raise IdentityError(f"{field} has an unsafe identifier")
    return value


def validate_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    if manifest.get("schema_version") != 1:
        raise IdentityError("unsupported identity manifest schema version")
    if manifest.get("gate_id") != "ghostrace-identity-gate-v1":
        raise IdentityError("unexpected identity gate id")
    if manifest.get("project") != "GHOSTRACE":
        raise IdentityError("project must be GHOSTRACE")
    if manifest.get("repository") != "AlisinaDevelo/GHOSTRACE":
        raise IdentityError("repository must be AlisinaDevelo/GHOSTRACE")
    if manifest.get("owner") != "AlisinaDevelo":
        raise IdentityError("owner must be AlisinaDevelo")
    as_of = require_string(manifest.get("as_of"), "as_of")
    if not DATE_RE.fullmatch(as_of):
        raise IdentityError("as_of must be YYYY-MM-DD")
    try:
        date.fromisoformat(as_of)
    except ValueError as exc:
        raise IdentityError("as_of is not a valid calendar date") from exc
    descriptor = require_string(manifest.get("descriptor"), "descriptor")
    if descriptor != "GHOSTRACE — local macOS event provenance journal":
        raise IdentityError("descriptor is not the qualified product descriptor")

    decision = manifest.get("decision")
    if not isinstance(decision, dict):
        raise IdentityError("decision must be an object")
    if decision.get("outcome") != "retain-qualified-descriptor-and-rename-distribution-identifiers":
        raise IdentityError("decision must record the qualified-descriptor outcome")
    if decision.get("legal_status") != "not_cleared":
        raise IdentityError("legal status must remain not_cleared")
    if decision.get("qualified_counsel_required") is not True:
        raise IdentityError("qualified counsel must remain required")
    require_string(decision.get("distribution_gate"), "decision.distribution_gate")
    rationale = decision.get("rationale")
    if not isinstance(rationale, list) or len(rationale) < 2 or any(not isinstance(item, str) or not item for item in rationale):
        raise IdentityError("decision.rationale must contain at least two statements")

    identifiers = manifest.get("identifiers")
    if not isinstance(identifiers, dict):
        raise IdentityError("identifiers must be an object")
    current = identifiers.get("current_development")
    if not isinstance(current, dict) or current.get("publication") != "disabled":
        raise IdentityError("current development publication must remain disabled")
    validate_identifier(current.get("binary"), "identifiers.current_development.binary", PACKAGE_RE)
    validate_identifier(current.get("crate"), "identifiers.current_development.crate", PACKAGE_RE)
    release = identifiers.get("release")
    if not isinstance(release, dict):
        raise IdentityError("identifiers.release must be an object")
    for key in ("binary", "crate", "homebrew_formula"):
        value = validate_identifier(release.get(key), f"identifiers.release.{key}", PACKAGE_RE)
        if value != "ghostrace-journal":
            raise IdentityError(f"identifiers.release.{key} must be ghostrace-journal")
    bundle_name = require_string(release.get("bundle_display_name"), "identifiers.release.bundle_display_name")
    if bundle_name != "GHOSTRACE Journal.app":
        raise IdentityError("bundle display name must be GHOSTRACE Journal.app")
    for key in ("bundle_identifier", "reverse_dns_identifier"):
        value = validate_identifier(release.get(key), f"identifiers.release.{key}", REVERSE_DNS_RE)
        if value != "com.alisinadevelo.ghostrace.journal":
            raise IdentityError(f"identifiers.release.{key} must be the selected reverse-DNS identifier")
    require_string(identifiers.get("migration_boundary"), "identifiers.migration_boundary")

    collisions = manifest.get("collision_review")
    if not isinstance(collisions, list) or not collisions:
        raise IdentityError("collision_review must be a non-empty list")
    seen: set[str] = set()
    for entry in collisions:
        if not isinstance(entry, dict):
            raise IdentityError("each collision review entry must be an object")
        entry_id = require_string(entry.get("id"), "collision_review.id")
        if entry_id in seen:
            raise IdentityError(f"duplicate collision review entry: {entry_id}")
        seen.add(entry_id)
        require_string(entry.get("surface"), f"{entry_id}.surface")
        require_string(entry.get("query"), f"{entry_id}.query")
        state = require_string(entry.get("result_state"), f"{entry_id}.result_state")
        if state not in ALLOWED_RESULT_STATES:
            raise IdentityError(f"{entry_id}.result_state is not an allowed conservative state")
        if state in {"available", "cleared"}:
            raise IdentityError(f"{entry_id} makes an unsafe availability or clearance claim")
        require_string(entry.get("observed"), f"{entry_id}.observed")
        urls = entry.get("urls")
        if not isinstance(urls, list) or not urls or any(not URL_RE.fullmatch(item) for item in urls):
            raise IdentityError(f"{entry_id}.urls must contain HTTPS sources")
        require_string(entry.get("action"), f"{entry_id}.action")
        if entry_id == "github-search" and require_nonnegative_int(entry.get("observed_count"), f"{entry_id}.observed_count") < 1:
            raise IdentityError("GitHub collision count must be positive")
        if entry_id == "npm" and entry.get("observed_count") != 0:
            raise IdentityError("npm exact search must retain its zero result")
        if entry_id in {"crates-io", "homebrew", "pypi"}:
            if type(entry.get("http_status")) is not int:
                raise IdentityError(f"{entry_id}.http_status must be retained")
        if entry_id == "domains":
            records = entry.get("records")
            if not isinstance(records, list) or {item.get("domain") for item in records if isinstance(item, dict)} != {
                "ghostrace.com",
                "ghostrace.net",
                "ghostrace.org",
                "ghostrace.dev",
                "ghostrace.app",
                "ghostrace.io",
            }:
                raise IdentityError("domain review must cover all selected TLDs")
            for record in records:
                if not isinstance(record, dict):
                    raise IdentityError("domain records must be objects")
                require_string(record.get("domain"), "domain.domain")
                record_state = require_string(record.get("result_state"), "domain.result_state")
                if record_state not in {"registered", "no_rdap_record_observed", "lookup_unavailable"}:
                    raise IdentityError("domain result must remain conservative")
                if type(record.get("http_status")) is not int:
                    raise IdentityError("domain HTTP status must be retained")
                require_url(record.get("url"), "domain.url")

    if seen != REQUIRED_COLLISIONS:
        missing = ", ".join(sorted(REQUIRED_COLLISIONS - seen))
        extra = ", ".join(sorted(seen - REQUIRED_COLLISIONS))
        raise IdentityError(f"collision review coverage mismatch; missing={missing or '-'} extra={extra or '-'}")

    legal = manifest.get("legal_review")
    if not isinstance(legal, dict) or legal.get("status") != "not_cleared" or legal.get("qualified_counsel_required") is not True:
        raise IdentityError("legal_review must remain unresolved and counsel-gated")
    for jurisdiction in ("uspto", "euipo"):
        record = legal.get(jurisdiction)
        if not isinstance(record, dict):
            raise IdentityError(f"legal_review.{jurisdiction} must be an object")
        require_url(record.get("official_search"), f"legal_review.{jurisdiction}.official_search")
        require_string(record.get("query"), f"legal_review.{jurisdiction}.query")
        require_bool(record.get("records_reviewed"), f"legal_review.{jurisdiction}.records_reviewed")
        if record["records_reviewed"] is not True:
            raise IdentityError(f"legal_review.{jurisdiction} records must be explicitly reviewed")
        require_nonnegative_int(record.get("record_count"), f"legal_review.{jurisdiction}.record_count")
        if record.get("result_state") != "unresolved_manual_review":
            raise IdentityError(f"legal_review.{jurisdiction} must remain unresolved_manual_review")
        require_string(record.get("observed"), f"legal_review.{jurisdiction}.observed")
        require_url(record.get("query_endpoint"), f"legal_review.{jurisdiction}.query_endpoint")
    require_string(legal.get("limitation"), "legal_review.limitation")
    rerun = manifest.get("rerun_policy")
    if not isinstance(rerun, dict):
        raise IdentityError("rerun_policy must be an object")
    required_before = rerun.get("required_before")
    if not isinstance(required_before, list) or len(required_before) < 2 or any(not isinstance(item, str) or not item for item in required_before):
        raise IdentityError("rerun_policy.required_before must list release boundaries")
    surfaces = rerun.get("surfaces")
    if not isinstance(surfaces, list) or len(surfaces) < 5 or any(not isinstance(item, str) or not item for item in surfaces):
        raise IdentityError("rerun_policy.surfaces must cover the audited providers")
    require_string(rerun.get("retained_data"), "rerun_policy.retained_data")
    return {
        "ok": True,
        "collision_sources": len(collisions),
        "legal_jurisdictions": 2,
        "release_identifier": release["binary"],
        "as_of": as_of,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check",))
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    args = parser.parse_args(argv)
    try:
        result = validate_manifest(load_manifest(args.manifest))
    except IdentityError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
