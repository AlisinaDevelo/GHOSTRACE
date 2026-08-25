#!/usr/bin/env python3
"""Validate and deterministically replay the FSEvents lifecycle corpus.

The corpus is a public, synthetic contract.  ``check`` never touches a live
filesystem, power state, session, or volume.  Its replay distribution is a
deterministic projection used to prove the fixture and reporting pipeline; a
macOS native integration receipt is required before any device-safe scenario is
called observed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_PATH = ROOT / "fixtures" / "fsevents-lifecycle-corpus-v1.json"
SCHEMA_VERSION = 1
GENERATOR_VERSION = "ghostrace-lifecycle-replay-v1"
EXPECTED_SEED = 7501
EXPECTED_COMMAND = "python3 scripts/fsevents-lifecycle-corpus.py check"
EXPECTED_SCENARIOS = [
    "bulk_checkout",
    "package_install",
    "rename_storm",
    "directory_deletion",
    "sleep_wake",
    "logout",
    "volume_detach",
    "process_kill",
    "restart",
]
SAFE_MODES = {"device_safe", "device_guarded"}
EXPECTED_METRICS = [
    "omission_rate",
    "duplicate_rate",
    "ordering_inversion_rate",
    "recovery_success_rate",
    "resource_peak_events",
]
OPERATION_RE = re.compile(r"^[a-z][a-z0-9_-]*$")
TOKEN_RE = re.compile(r"^[a-z][a-z0-9_-]*$")
FORBIDDEN_FIELDS = {"path", "content", "plaintext", "url", "command_line", "environment"}


class CorpusError(ValueError):
    """The lifecycle fixture or its deterministic report is invalid."""


def _read_fixture() -> dict[str, Any]:
    try:
        document = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise CorpusError("cannot read lifecycle corpus fixture") from exc
    if not isinstance(document, dict):
        raise CorpusError("lifecycle corpus root must be an object")
    return document


def _require_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise CorpusError(f"{field} must be a non-empty string")
    return value


def _require_positive_int(value: Any, field: str) -> int:
    if type(value) is not int or value <= 0:
        raise CorpusError(f"{field} must be a positive integer")
    return value


def _check_no_forbidden_fields(value: Any, location: str = "fixture") -> None:
    if isinstance(value, dict):
        forbidden = FORBIDDEN_FIELDS.intersection(value)
        if forbidden:
            raise CorpusError(f"{location} contains forbidden fields: {sorted(forbidden)}")
        for key, child in value.items():
            _check_no_forbidden_fields(child, f"{location}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _check_no_forbidden_fields(child, f"{location}[{index}]")


def _validate_scenario(scenario: Any, index: int, max_operations: int, max_events: int) -> dict[str, Any]:
    field = f"scenarios[{index}]"
    if not isinstance(scenario, dict):
        raise CorpusError(f"{field} must be an object")
    required = {
        "id",
        "mode",
        "native_test",
        "ground_truth",
        "expected_direct",
        "permitted_coalescing",
        "required_gaps",
        "recovery",
        "resource_budget",
    }
    if set(scenario) - required - {"no_go_reason"} or not required.issubset(scenario):
        raise CorpusError(f"{field} has an invalid schema")
    scenario_id = _require_string(scenario["id"], f"{field}.id")
    if scenario_id != EXPECTED_SCENARIOS[index]:
        raise CorpusError(f"{field}.id must be {EXPECTED_SCENARIOS[index]}")
    mode = scenario["mode"]
    if mode not in SAFE_MODES:
        raise CorpusError(f"{field}.mode is invalid")
    native_test = scenario["native_test"]
    if native_test not in {"native_safe", "guarded_no_go"}:
        raise CorpusError(f"{field}.native_test is invalid")
    if (mode == "device_safe") != (native_test == "native_safe"):
        raise CorpusError(f"{field} mode/native_test mismatch")
    no_go_reason = scenario.get("no_go_reason")
    if mode == "device_guarded":
        _require_string(no_go_reason, f"{field}.no_go_reason")
    elif no_go_reason is not None:
        raise CorpusError(f"{field}.no_go_reason is only valid for guarded scenarios")

    ground_truth = scenario["ground_truth"]
    if not isinstance(ground_truth, list) or not ground_truth:
        raise CorpusError(f"{field}.ground_truth must be a non-empty list")
    if len(ground_truth) > max_operations:
        raise CorpusError(f"{field}.ground_truth exceeds the corpus operation bound")
    expected_sequence = list(range(1, len(ground_truth) + 1))
    actual_sequence: list[int] = []
    operations: list[str] = []
    for operation_index, entry in enumerate(ground_truth):
        entry_field = f"{field}.ground_truth[{operation_index}]"
        if not isinstance(entry, dict) or set(entry) != {"seq", "operation", "token"}:
            raise CorpusError(f"{entry_field} has an invalid schema")
        sequence = entry["seq"]
        if type(sequence) is not int:
            raise CorpusError(f"{entry_field}.seq must be an integer")
        actual_sequence.append(sequence)
        operation = _require_string(entry["operation"], f"{entry_field}.operation")
        token = _require_string(entry["token"], f"{entry_field}.token")
        if OPERATION_RE.fullmatch(operation) is None or TOKEN_RE.fullmatch(token) is None:
            raise CorpusError(f"{entry_field} contains an unsafe identifier")
        operations.append(operation)
    if actual_sequence != expected_sequence:
        raise CorpusError(f"{field}.ground_truth sequence must be contiguous from one")

    direct = scenario["expected_direct"]
    if not isinstance(direct, list) or any(not isinstance(item, str) for item in direct):
        raise CorpusError(f"{field}.expected_direct must be a string list")
    if len(set(direct)) != len(direct) or any(item not in operations for item in direct):
        raise CorpusError(f"{field}.expected_direct must name unique ground-truth operations")
    for list_field in ("permitted_coalescing", "required_gaps"):
        values = scenario[list_field]
        if not isinstance(values, list) or any(
            not isinstance(item, str) or not item for item in values
        ):
            raise CorpusError(f"{field}.{list_field} must be a string list")
        if len(set(values)) != len(values):
            raise CorpusError(f"{field}.{list_field} must not contain duplicates")
    _require_string(scenario["recovery"], f"{field}.recovery")

    budget = scenario["resource_budget"]
    if not isinstance(budget, dict) or set(budget) != {"max_operations", "max_observed_events"}:
        raise CorpusError(f"{field}.resource_budget has an invalid schema")
    budget_operations = _require_positive_int(budget["max_operations"], f"{field}.resource_budget.max_operations")
    budget_events = _require_positive_int(budget["max_observed_events"], f"{field}.resource_budget.max_observed_events")
    if len(ground_truth) > budget_operations or budget_events > max_events:
        raise CorpusError(f"{field}.resource_budget exceeds the corpus limits")
    if mode == "device_guarded" and not scenario["required_gaps"]:
        raise CorpusError(f"{field} must require a coverage gap")
    if mode == "device_safe" and scenario["required_gaps"]:
        raise CorpusError(f"{field} cannot require a guarded lifecycle gap")
    return {"id": scenario_id, "mode": mode, "ground_truth": ground_truth, "direct": direct, "budget": budget}


def validate(document: dict[str, Any]) -> list[dict[str, Any]]:
    _check_no_forbidden_fields(document)
    required = {
        "schema_version",
        "program",
        "generator",
        "privacy",
        "repeat_runs",
        "resource_limits",
        "distribution_expectations",
        "scenarios",
    }
    if set(document) != required:
        raise CorpusError(f"fixture keys must be {sorted(required)}")
    if document["schema_version"] != SCHEMA_VERSION:
        raise CorpusError("unsupported lifecycle corpus schema")
    if document["program"] != "ghostrace-fsevents-lifecycle-corpus-v1":
        raise CorpusError("program identifier drifted")
    generator = document["generator"]
    if not isinstance(generator, dict) or set(generator) != {"version", "seed", "algorithm", "command"}:
        raise CorpusError("generator has an invalid schema")
    if generator["version"] != GENERATOR_VERSION or generator["seed"] != EXPECTED_SEED:
        raise CorpusError("generator version or seed drifted")
    if generator["algorithm"] != "sha256(seed:scenario:run) deterministic projection; no runtime randomness":
        raise CorpusError("generator algorithm drifted")
    if generator["command"] != EXPECTED_COMMAND:
        raise CorpusError("generator command drifted")
    privacy = document["privacy"]
    if privacy != {"synthetic_only": True, "user_data_included": False, "network_required": False}:
        raise CorpusError("lifecycle corpus must remain synthetic, private, and offline")
    repeat_runs = _require_positive_int(document["repeat_runs"], "repeat_runs")
    if repeat_runs < 32:
        raise CorpusError("repeat_runs must retain at least 32 deterministic replays")
    limits = document["resource_limits"]
    if not isinstance(limits, dict) or set(limits) != {
        "max_ground_truth_operations",
        "max_observed_events",
        "max_callback_batch",
        "max_run_ms",
    }:
        raise CorpusError("resource_limits has an invalid schema")
    max_operations = _require_positive_int(limits["max_ground_truth_operations"], "resource_limits.max_ground_truth_operations")
    max_events = _require_positive_int(limits["max_observed_events"], "resource_limits.max_observed_events")
    _require_positive_int(limits["max_callback_batch"], "resource_limits.max_callback_batch")
    _require_positive_int(limits["max_run_ms"], "resource_limits.max_run_ms")
    expectations = document["distribution_expectations"]
    if not isinstance(expectations, dict) or set(expectations) != {
        "metrics",
        "min_replay_runs",
        "recovery_success_rate_min",
        "resource_peak_events_max",
    }:
        raise CorpusError("distribution_expectations has an invalid schema")
    if expectations["metrics"] != EXPECTED_METRICS:
        raise CorpusError("distribution metric order drifted")
    if expectations["min_replay_runs"] < repeat_runs:
        raise CorpusError("distribution gate requires every configured replay")
    if not isinstance(expectations["recovery_success_rate_min"], (int, float)):
        raise CorpusError("recovery_success_rate_min must be numeric")
    _require_positive_int(expectations["resource_peak_events_max"], "resource_peak_events_max")
    scenarios = document["scenarios"]
    if not isinstance(scenarios, list) or len(scenarios) != len(EXPECTED_SCENARIOS):
        raise CorpusError(f"scenarios must contain exactly {len(EXPECTED_SCENARIOS)} entries")
    return [_validate_scenario(item, index, max_operations, max_events) for index, item in enumerate(scenarios)]


def _replay(scenarios: list[dict[str, Any]], seed: int, runs: int, max_events: int) -> dict[str, Any]:
    reports: dict[str, Any] = {}
    totals = {metric: 0.0 for metric in EXPECTED_METRICS if metric != "resource_peak_events"}
    total_direct = 0
    total_observed = 0
    total_runs = runs * len(scenarios)
    recovery_successes = 0
    resource_peak = 0
    for scenario in scenarios:
        direct_operations = [
            entry for entry in scenario["ground_truth"] if entry["operation"] in scenario["direct"]
        ]
        direct_count = len(direct_operations)
        omissions = 0
        duplicates = 0
        inversions = 0
        observed_peak = 0
        recoveries = 0
        for run in range(1, runs + 1):
            digest = hashlib.sha256(f"{seed}:{scenario['id']}:{run}".encode("ascii")).digest()
            omitted = 1 if direct_count and digest[0] % 17 == 0 else 0
            duplicate = 1 if direct_count and digest[1] % 13 == 0 else 0
            inversion = 1 if direct_count > 1 and digest[2] % 19 == 0 else 0
            observed = max(direct_count - omitted + duplicate, 0)
            if observed > scenario["budget"]["max_observed_events"] or observed > max_events:
                raise CorpusError(f"replay exceeds resource bound for {scenario['id']}")
            omissions += omitted
            duplicates += duplicate
            inversions += inversion
            observed_peak = max(observed_peak, observed)
            recoveries += 1
        total_direct += direct_count * runs
        total_observed += max(direct_count * runs - omissions + duplicates, 0)
        resource_peak = max(resource_peak, observed_peak)
        recovery_successes += recoveries
        reports[scenario["id"]] = {
            "mode": scenario["mode"],
            "runs": runs,
            "ground_truth_direct_operations": direct_count,
            "omissions": omissions,
            "duplicates": duplicates,
            "ordering_inversions": inversions,
            "recovery_successes": recoveries,
            "omission_rate": round(omissions / (direct_count * runs), 8) if direct_count else 0.0,
            "duplicate_rate": round(duplicates / (direct_count * runs), 8) if direct_count else 0.0,
            "ordering_inversion_rate": round(inversions / runs, 8),
            "recovery_success_rate": round(recoveries / runs, 8),
            "resource_peak_events": observed_peak,
            "interpretation": "fixture_replay_only",
        }
    denominator = total_direct or 1
    totals["omission_rate"] = round(sum(item["omissions"] for item in reports.values()) / denominator, 8)
    totals["duplicate_rate"] = round(sum(item["duplicates"] for item in reports.values()) / denominator, 8)
    totals["ordering_inversion_rate"] = round(
        sum(item["ordering_inversions"] for item in reports.values()) / total_runs, 8
    )
    totals["recovery_success_rate"] = round(recovery_successes / total_runs, 8)
    return {
        "runs": runs,
        "direct_operations": total_direct,
        "observed_events": total_observed,
        "omissions": sum(item["omissions"] for item in reports.values()),
        "duplicates": sum(item["duplicates"] for item in reports.values()),
        "ordering_inversions": sum(item["ordering_inversions"] for item in reports.values()),
        "replay_distribution": {
            **totals,
            "resource_peak_events": resource_peak,
            "scenarios": reports,
        },
    }


def report(document: dict[str, Any]) -> dict[str, Any]:
    scenarios = validate(document)
    replay = _replay(scenarios, document["generator"]["seed"], document["repeat_runs"], document["resource_limits"]["max_observed_events"])
    expectations = document["distribution_expectations"]
    distribution = replay["replay_distribution"]
    if distribution["recovery_success_rate"] < expectations["recovery_success_rate_min"]:
        raise CorpusError("replay recovery distribution is below the configured minimum")
    if distribution["resource_peak_events"] > expectations["resource_peak_events_max"]:
        raise CorpusError("replay resource distribution exceeds the configured maximum")
    native = [scenario["id"] for scenario in scenarios if scenario["mode"] == "device_safe"]
    guarded = [scenario["id"] for scenario in scenarios if scenario["mode"] == "device_guarded"]
    return {
        "ok": True,
        "schema_version": document["schema_version"],
        "repeat_runs": document["repeat_runs"],
        "scenario_count": len(scenarios),
        "native_device_scenarios": native,
        "guarded_no_go_scenarios": guarded,
        "metrics": document["distribution_expectations"]["metrics"],
        "replay_distribution": distribution,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check",))
    args = parser.parse_args(argv)
    try:
        result = report(_read_fixture())
    except (CorpusError, OSError, json.JSONDecodeError) as exc:
        print(f"fsevents-lifecycle-corpus: {exc}", file=sys.stderr)
        return 1
    if args.command == "check":
        print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
