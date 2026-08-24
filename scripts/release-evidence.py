#!/usr/bin/env python3
"""Validate and gate the GHOSTRACE release evidence register.

The register is intentionally conservative: planned, inferred, unavailable,
missing, stale, or narrow-scope evidence blocks a release gate. This tool never
turns an absent measurement into a pass.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import date, timedelta
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REGISTER = ROOT / "planning" / "release-evidence-register.json"
VALID_STATES = {"planned", "observed", "inferred", "unavailable"}
VALID_KINDS = {"binary", "count", "threshold", "rate"}


class EvidenceError(ValueError):
    """The register is malformed or cannot satisfy a gate."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise EvidenceError(f"cannot read register: {path.name}") from exc
    if not isinstance(value, dict):
        raise EvidenceError("register root must be an object")
    return value


def parse_date(value: Any, field: str) -> date:
    if not isinstance(value, str):
        raise EvidenceError(f"{field} must be an ISO date")
    try:
        return date.fromisoformat(value)
    except ValueError as exc:
        raise EvidenceError(f"{field} must be an ISO date") from exc


def nonempty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise EvidenceError(f"{field} must be a non-empty string")
    return value


def scope_list(value: Any, field: str) -> list[str]:
    if not isinstance(value, list) or not value or any(not isinstance(item, str) or not item for item in value):
        raise EvidenceError(f"{field} must be a non-empty string list")
    return value


def validate_register(register: dict[str, Any]) -> dict[str, Any]:
    if register.get("schema_version") != 1:
        raise EvidenceError("unsupported register schema version")
    if register.get("required_evidence_state") != "observed":
        raise EvidenceError("release gates must require observed evidence")
    states = register.get("states")
    if states != ["planned", "observed", "inferred", "unavailable"]:
        raise EvidenceError("register states must preserve all four evidence states")
    policy = register.get("gate_policy")
    if not isinstance(policy, dict):
        raise EvidenceError("gate_policy must be an object")
    for key in ("require_artifact_exists", "require_freshness", "require_scope_coverage"):
        if policy.get(key) is not True:
            raise EvidenceError(f"gate_policy.{key} must be true")
    if policy.get("allow_inferred_for_release") is not False or policy.get("allow_unavailable_for_release") is not False:
        raise EvidenceError("inferred and unavailable evidence must block release")

    program = load_json(ROOT / "planning" / "program.json")
    expected_due = {
        item.get("title"): item.get("due_on", "").split("T", 1)[0]
        for item in program.get("milestones", [])
        if isinstance(item, dict)
    }
    milestones = register.get("milestones")
    if not isinstance(milestones, list):
        raise EvidenceError("milestones must be a list")
    seen: set[str] = set()
    measure_count = 0
    state_counts = {state: 0 for state in VALID_STATES}
    for milestone in milestones:
        if not isinstance(milestone, dict):
            raise EvidenceError("each milestone must be an object")
        milestone_id = nonempty_string(milestone.get("id"), "milestone.id")
        if milestone_id in seen:
            raise EvidenceError(f"duplicate milestone {milestone_id}")
        seen.add(milestone_id)
        if milestone_id not in expected_due:
            raise EvidenceError(f"unknown milestone {milestone_id}")
        if milestone.get("due_on") != expected_due[milestone_id]:
            raise EvidenceError(f"{milestone_id}.due_on does not match planning/program.json")
        nonempty_string(milestone.get("outcome"), f"{milestone_id}.outcome")
        measures = milestone.get("exit_measures")
        if not isinstance(measures, list) or not measures:
            raise EvidenceError(f"{milestone_id} must have exit measures")
        measure_ids: set[str] = set()
        for measure in measures:
            if not isinstance(measure, dict):
                raise EvidenceError(f"{milestone_id} measure must be an object")
            measure_id = nonempty_string(measure.get("id"), f"{milestone_id}.measure.id")
            if measure_id in measure_ids:
                raise EvidenceError(f"duplicate measure {measure_id}")
            measure_ids.add(measure_id)
            if measure.get("kind") not in VALID_KINDS:
                raise EvidenceError(f"{measure_id}.kind is invalid")
            nonempty_string(measure.get("target"), f"{measure_id}.target")
            nonempty_string(measure.get("measure"), f"{measure_id}.measure")
            artifact = nonempty_string(measure.get("artifact"), f"{measure_id}.artifact")
            if not artifact.startswith(("docs/", "artifacts/")) or Path(artifact).is_absolute():
                raise EvidenceError(f"{measure_id}.artifact must be a repository-relative evidence path")
            scope_list(measure.get("scope"), f"{measure_id}.scope")
            freshness = measure.get("freshness_days")
            if type(freshness) is not int or freshness <= 0:
                raise EvidenceError(f"{measure_id}.freshness_days must be positive")
            evidence = measure.get("evidence")
            if not isinstance(evidence, dict):
                raise EvidenceError(f"{measure_id}.evidence must be an object")
            state = evidence.get("state")
            if state not in VALID_STATES:
                raise EvidenceError(f"{measure_id}.evidence.state is invalid")
            evidence_artifact = nonempty_string(evidence.get("artifact"), f"{measure_id}.evidence.artifact")
            if evidence_artifact != artifact:
                raise EvidenceError(f"{measure_id} evidence artifact does not match the required artifact")
            evidence_scope = scope_list(evidence.get("scope"), f"{measure_id}.evidence.scope")
            if state == "observed":
                parse_date(evidence.get("observed_at"), f"{measure_id}.evidence.observed_at")
            elif evidence.get("observed_at") is not None:
                raise EvidenceError(f"{measure_id} non-observed evidence cannot have observed_at")
            nonempty_string(evidence.get("notes"), f"{measure_id}.evidence.notes")
            state_counts[state] += 1
            measure_count += 1
            if not set(scope_list(measure.get("scope"), f"{measure_id}.scope")).issubset(evidence_scope):
                raise EvidenceError(f"{measure_id} evidence scope is narrower than the required scope")

    expected_ids = set(expected_due)
    if seen != expected_ids:
        missing = ", ".join(sorted(expected_ids - seen))
        extra = ", ".join(sorted(seen - expected_ids))
        raise EvidenceError(f"milestone coverage mismatch; missing={missing or '-'} extra={extra or '-'}")
    return {"milestones": len(milestones), "measures": measure_count, "state_counts": state_counts}


def gate(register: dict[str, Any], milestone_id: str, as_of: date) -> dict[str, Any]:
    milestone = next((item for item in register["milestones"] if item["id"] == milestone_id), None)
    if milestone is None:
        raise EvidenceError(f"unknown milestone {milestone_id}")
    blockers: list[dict[str, str]] = []
    for measure in milestone["exit_measures"]:
        evidence = measure["evidence"]
        state = evidence["state"]
        if state != "observed":
            blockers.append({"measure": measure["id"], "code": f"state_{state}", "detail": "release gates require observed evidence"})
            continue
        observed_at = parse_date(evidence["observed_at"], f"{measure['id']}.evidence.observed_at")
        if observed_at > as_of:
            blockers.append({"measure": measure["id"], "code": "future_observation", "detail": "observation is after the gate date"})
        expires = observed_at + timedelta(days=measure["freshness_days"])
        if expires < as_of:
            blockers.append({"measure": measure["id"], "code": "stale", "detail": f"evidence expired on {expires.isoformat()}"})
        if not (ROOT / evidence["artifact"]).is_file():
            blockers.append({"measure": measure["id"], "code": "missing_artifact", "detail": "required evidence artifact is absent"})
        if not set(measure["scope"]).issubset(set(evidence["scope"])):
            blockers.append({"measure": measure["id"], "code": "narrow_scope", "detail": "evidence scope does not cover the measure"})
    return {"milestone": milestone_id, "as_of": as_of.isoformat(), "allowed": not blockers, "blockers": blockers}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "gate"))
    parser.add_argument("--register", type=Path, default=DEFAULT_REGISTER)
    parser.add_argument("--milestone")
    parser.add_argument("--as-of", default=date.today().isoformat())
    args = parser.parse_args(argv)
    try:
        register = load_json(args.register)
        summary = validate_register(register)
        if args.command == "check":
            print(json.dumps({"ok": True, **summary}, sort_keys=True))
            return 0
        if not args.milestone:
            raise EvidenceError("gate requires --milestone")
        result = gate(register, args.milestone, parse_date(args.as_of, "--as-of"))
        print(json.dumps(result, sort_keys=True))
        return 0 if result["allowed"] else 1
    except EvidenceError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, sort_keys=True))
        return 2


if __name__ == "__main__":
    sys.exit(main())
