#!/usr/bin/env python3
"""Check the schema and export compatibility matrix.

`planning/compatibility-matrix.json` lists every retained public format with
its supported read and write versions, a golden artifact, and an explicit
`accept`, `accept_with_upgrade`, or `refuse` outcome for each compatibility
case. Every outcome names the test that proves it; the checker fails if that
test function does not exist. A deprecated format or a retired version must
carry the full deprecation record. Uses only the standard library.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = ROOT / "planning" / "compatibility-matrix.json"

OUTCOMES = {"accept", "accept_with_upgrade", "refuse"}
REQUIRED_CASES = {
    "json_contract": {"current", "forward", "backward", "unknown_field", "corrupted"},
    "stream": {"current", "forward", "backward", "unknown_field", "mixed", "corrupted"},
    "database": {"current", "backward", "forward", "downgrade", "partially_migrated", "corrupted"},
}
# Cases that must never be accepted: accepting them would read data the
# format cannot represent or trust.
MUST_REFUSE = {"forward", "unknown_field", "mixed", "corrupted", "partially_migrated", "downgrade"}
FORMAT_FIELDS = {
    "id",
    "kind",
    "format",
    "current_version",
    "supported_read",
    "write_version",
    "golden",
    "status",
    "cases",
}
DEPRECATION_FIELDS = {"announced_in", "removal_not_before", "migration_tool", "rollback_evidence", "release_note"}


class MatrixError(ValueError):
    """The compatibility matrix is invalid or unproven."""


def load(path: Path = MATRIX_PATH) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise MatrixError(f"cannot read {path.name}") from exc


def test_exists(reference: str, root: Path) -> bool:
    if "::" not in reference:
        return False
    file_name, function = reference.split("::", 1)
    path = root / file_name
    if not path.is_file() or not re.fullmatch(r"[a-z0-9_]+", function):
        return False
    return re.search(rf"\bfn {function}\s*\(", path.read_text(encoding="utf-8")) is not None


def validate_deprecation(record: Any, label: str, root: Path) -> None:
    if type(record) is not dict or set(record) != DEPRECATION_FIELDS:
        raise MatrixError(f"{label}: deprecation record must have {sorted(DEPRECATION_FIELDS)}")
    for field in DEPRECATION_FIELDS:
        if type(record[field]) is not str or not record[field].strip():
            raise MatrixError(f"{label}: deprecation {field} is empty")
    for field in ("migration_tool", "rollback_evidence"):
        target = record[field].split("::", 1)[0]
        if not (root / target).exists():
            raise MatrixError(f"{label}: deprecation {field} does not exist")


def validate(document: Any, root: Path = ROOT) -> dict[str, Any]:
    if type(document) is not dict or set(document) != {"schema_version", "deprecation_policy", "formats", "retired"}:
        raise MatrixError("compatibility matrix has an invalid schema")
    if document["schema_version"] != 1:
        raise MatrixError("unsupported compatibility matrix version")
    policy = document["deprecation_policy"]
    if type(policy) is not dict or set(policy.get("requires", [])) != DEPRECATION_FIELDS:
        raise MatrixError("deprecation policy must require the full deprecation record")
    if type(policy.get("minimum_window_releases")) is not int or policy["minimum_window_releases"] < 1:
        raise MatrixError("deprecation window must be at least one release")
    formats = document["formats"]
    if type(formats) is not list or not formats:
        raise MatrixError("formats must be a non-empty list")
    seen: set[str] = set()
    proofs = 0
    for entry in formats:
        if type(entry) is not dict or set(entry) != FORMAT_FIELDS | ({"deprecation"} & set(entry)):
            raise MatrixError("format entry has an invalid schema")
        label = entry["id"]
        if label in seen:
            raise MatrixError(f"duplicate format {label}")
        seen.add(label)
        if entry["kind"] not in REQUIRED_CASES:
            raise MatrixError(f"{label}: unknown kind")
        reads = entry["supported_read"]
        if type(reads) is not list or not reads or entry["current_version"] not in reads:
            raise MatrixError(f"{label}: the current version must be readable")
        if entry["write_version"] != entry["current_version"]:
            raise MatrixError(f"{label}: writers must emit the current version")
        if not (root / entry["golden"]).is_file():
            raise MatrixError(f"{label}: golden artifact is missing")
        if entry["status"] == "deprecated":
            validate_deprecation(entry.get("deprecation"), label, root)
        elif entry["status"] != "supported" or "deprecation" in entry:
            raise MatrixError(f"{label}: status must be supported or deprecated")
        cases = entry["cases"]
        missing = REQUIRED_CASES[entry["kind"]] - set(cases)
        if missing:
            raise MatrixError(f"{label}: missing cases {sorted(missing)}")
        for name, case in cases.items():
            if type(case) is not dict or set(case) != {"outcome", "test"}:
                raise MatrixError(f"{label}.{name}: invalid case")
            if case["outcome"] not in OUTCOMES:
                raise MatrixError(f"{label}.{name}: unknown outcome")
            if name in MUST_REFUSE and case["outcome"] != "refuse":
                raise MatrixError(f"{label}.{name}: must be refused")
            if name == "current" and case["outcome"] != "accept":
                raise MatrixError(f"{label}.current: the golden artifact must be accepted")
            if not test_exists(case["test"], root):
                raise MatrixError(f"{label}.{name}: proving test {case['test']} does not exist")
            proofs += 1
    retired = document["retired"]
    if type(retired) is not list:
        raise MatrixError("retired must be a list")
    for item in retired:
        if type(item) is not dict or set(item) != {"id", "version", "deprecation"}:
            raise MatrixError("retired entry has an invalid schema")
        validate_deprecation(item["deprecation"], f"retired {item['id']} v{item['version']}", root)
        entry = next((entry for entry in formats if entry["id"] == item["id"]), None)
        if entry is not None and item["version"] in entry["supported_read"]:
            raise MatrixError(f"retired {item['id']} v{item['version']} is still listed as readable")
    return {"formats": len(formats), "proven_cases": proofs, "retired": len(retired), "ok": True}


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if args != ["check"]:
        print("usage: python3 scripts/compatibility.py check", file=sys.stderr)
        return 2
    try:
        report = validate(load())
    except MatrixError as exc:
        print(f"compatibility: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
