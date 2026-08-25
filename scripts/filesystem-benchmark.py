#!/usr/bin/env python3
"""Validate and analyze the synthetic filesystem benchmark contract.

The native macOS workload is implemented in ``tests/filesystem_benchmark.rs`` so
the same source revision can be exercised by Cargo on the target device.  This
script owns the strict fixture contract and the path/content-free report math.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CORPUS_PATH = ROOT / "fixtures" / "filesystem-benchmark-corpus-v1.json"
EXPECTED_SCENARIOS = [
    "small_tree",
    "deep_tree",
    "wide_tree",
    "unicode_tree",
    "case_variant_tree",
    "git_tree",
    "build_output_tree",
    "event_storm_tree",
]
EXPECTED_METRICS = [
    "latency_ms",
    "coverage_classes",
    "duplicate_rate",
    "gap_rate",
    "cpu_user_ms",
    "cpu_system_ms",
    "rss_peak_bytes",
    "energy_nj",
    "disk_growth_bytes",
]
FORBIDDEN_KEYS = {
    "path",
    "root_path",
    "file_path",
    "content",
    "payload",
    "user_data",
    "username",
    "hostname",
    "serial",
}


class BenchmarkError(ValueError):
    """A malformed or privacy-unsafe benchmark artifact."""


def _load_json(value: str, description: str) -> Any:
    try:
        return json.loads(value)
    except json.JSONDecodeError as exc:
        raise BenchmarkError(f"{description} is not valid JSON: {exc}") from exc


def _read_corpus() -> dict[str, Any]:
    try:
        document = _load_json(CORPUS_PATH.read_text(encoding="utf-8"), "corpus")
    except OSError as exc:
        raise BenchmarkError(f"cannot read corpus: {exc}") from exc
    if not isinstance(document, dict):
        raise BenchmarkError("corpus must be an object")
    return document


def _walk_forbidden(value: Any, location: str = "root") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key.lower() in FORBIDDEN_KEYS:
                raise BenchmarkError(f"forbidden field {location}.{key}")
            _walk_forbidden(child, f"{location}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _walk_forbidden(child, f"{location}[{index}]")
    elif isinstance(value, str):
        lowered = value.lower()
        if any(marker in lowered for marker in ("/users/", "/home/", "\\users\\", "secret", "password")):
            raise BenchmarkError(f"forbidden user-data-like value at {location}")


def validate_corpus(document: dict[str, Any]) -> dict[str, Any]:
    _walk_forbidden(document)
    required = {
        "schema_version",
        "program",
        "generator",
        "repeat_runs",
        "platform_no_go",
        "privacy",
        "metrics",
        "resource_limits",
        "scenarios",
    }
    if set(document) != required:
        raise BenchmarkError(f"corpus keys must be {sorted(required)}")
    if document["schema_version"] != 1 or document["program"] != "ghostrace-filesystem-benchmark-v1":
        raise BenchmarkError("unsupported benchmark corpus identity")
    if document["repeat_runs"] != 3:
        raise BenchmarkError("benchmark repeat_runs must remain exactly three")
    generator = document["generator"]
    if not isinstance(generator, dict) or set(generator) != {"version", "seed", "algorithm", "command"}:
        raise BenchmarkError("generator schema drifted")
    if generator != {
        "version": 1,
        "seed": "ghostrace-filesystem-benchmark-seed-v1",
        "algorithm": "deterministic-tree-v1",
        "command": "python3 scripts/filesystem-benchmark.py check",
    }:
        raise BenchmarkError("generator is not pinned")
    privacy = document["privacy"]
    if not isinstance(privacy, dict) or privacy != {
        "synthetic_only": True,
        "network": False,
        "reads_file_contents": False,
        "retains_paths": False,
        "max_path_bytes": 512,
        "max_file_bytes": 4096,
    }:
        raise BenchmarkError("privacy contract drifted")
    if document["metrics"] != EXPECTED_METRICS:
        raise BenchmarkError("metric order drifted")
    limits = document["resource_limits"]
    if not isinstance(limits, dict) or limits != {
        "max_entries": 512,
        "max_file_bytes": 4096,
        "max_run_ms": 30000,
        "max_journal_growth_bytes": 8388608,
    }:
        raise BenchmarkError("resource limits drifted")
    scenarios = document["scenarios"]
    if not isinstance(scenarios, list) or [item.get("id") for item in scenarios] != EXPECTED_SCENARIOS:
        raise BenchmarkError("scenario order or identity drifted")
    for index, scenario in enumerate(scenarios):
        if not isinstance(scenario, dict):
            raise BenchmarkError(f"scenario {index} is not an object")
        required_scenario = {
            "id",
            "mode",
            "native_test",
            "tree_shape",
            "expected_operations_min",
            "expected_operations_max",
            "expected_entry_min",
            "expected_entry_max",
            "mutations",
            "coverage_classes",
            "resource_budget",
        }
        if set(scenario) != required_scenario:
            raise BenchmarkError(f"scenario {index} schema drifted")
        if scenario["mode"] != "device_safe" or scenario["native_test"] != "native_safe":
            raise BenchmarkError(f"scenario {scenario['id']} must be native-safe")
        if not isinstance(scenario["mutations"], list) or not scenario["mutations"]:
            raise BenchmarkError(f"scenario {scenario['id']} has no mutations")
        if scenario["expected_operations_min"] > scenario["expected_operations_max"]:
            raise BenchmarkError(f"scenario {scenario['id']} operation bounds inverted")
        if scenario["expected_entry_min"] > scenario["expected_entry_max"]:
            raise BenchmarkError(f"scenario {scenario['id']} entry bounds inverted")
        if scenario["resource_budget"]["max_entries"] > limits["max_entries"]:
            raise BenchmarkError(f"scenario {scenario['id']} exceeds corpus entry bound")
        if scenario["resource_budget"]["max_run_ms"] > limits["max_run_ms"]:
            raise BenchmarkError(f"scenario {scenario['id']} exceeds corpus time bound")
    return {
        "ok": True,
        "schema_version": document["schema_version"],
        "scenario_count": len(scenarios),
        "scenario_ids": EXPECTED_SCENARIOS,
        "repeat_runs": document["repeat_runs"],
        "metrics": EXPECTED_METRICS,
        "privacy": privacy,
    }


def _number(value: Any, name: str, *, minimum: float = 0.0) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise BenchmarkError(f"{name} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < minimum:
        raise BenchmarkError(f"{name} is outside its finite bound")
    return number


def _percentile(values: list[float], percentile: float) -> float:
    ordered = sorted(values)
    rank = max(1, math.ceil(len(ordered) * percentile / 100.0)) - 1
    return ordered[min(rank, len(ordered) - 1)]


def report_from_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    _walk_forbidden(receipt)
    required = {"schema_version", "source_revision", "device", "latency_samples_ms", "scenarios", "resource"}
    if set(receipt) - required:
        raise BenchmarkError("receipt contains unsupported or privacy-sensitive fields")
    if receipt.get("schema_version") != 1:
        raise BenchmarkError("unsupported receipt schema")
    revision = receipt.get("source_revision")
    if not isinstance(revision, str) or len(revision) != 40 or any(char not in "0123456789abcdef" for char in revision):
        raise BenchmarkError("source_revision must be a lowercase commit SHA")
    device = receipt.get("device")
    if not isinstance(device, dict) or set(device) != {"model", "os", "arch", "toolchain"}:
        raise BenchmarkError("device context must name model, OS, architecture, and toolchain")
    if any(not isinstance(value, str) or not value.strip() for value in device.values()):
        raise BenchmarkError("device context values must be non-empty strings")
    samples = receipt.get("latency_samples_ms")
    if not isinstance(samples, list) or not samples:
        raise BenchmarkError("latency_samples_ms must be non-empty")
    latencies = [_number(value, "latency sample") for value in samples]
    scenarios = receipt.get("scenarios")
    if not isinstance(scenarios, list):
        raise BenchmarkError("scenarios must be a list")
    coverage = {name: 0 for name in ("direct", "contextual", "inferred", "unknown")}
    duplicates = 0
    observed = 0
    gaps = 0
    expected_operations = 0
    failures: dict[str, int] = {}
    for index, scenario in enumerate(scenarios):
        if not isinstance(scenario, dict) or set(scenario) != {
            "id",
            "coverage",
            "expected_operations",
            "observed_events",
            "duplicates",
            "gaps",
            "errors",
            "latency_ms",
        }:
            raise BenchmarkError(f"scenario receipt {index} schema drifted")
        if scenario["id"] not in EXPECTED_SCENARIOS:
            raise BenchmarkError(f"unknown scenario receipt {scenario['id']}")
        classes = scenario["coverage"]
        if not isinstance(classes, dict) or set(classes) != set(coverage):
            raise BenchmarkError(f"scenario receipt {scenario['id']} coverage classes drifted")
        for name, count in classes.items():
            if not isinstance(count, int) or count < 0:
                raise BenchmarkError(f"scenario receipt {scenario['id']} has invalid {name} count")
            coverage[name] += count
        for field in ("expected_operations", "observed_events", "duplicates", "gaps"):
            if not isinstance(scenario[field], int) or scenario[field] < 0:
                raise BenchmarkError(f"scenario receipt {scenario['id']} has invalid {field}")
        if not isinstance(scenario["errors"], list) or any(
            not isinstance(error, str) or not error.strip() for error in scenario["errors"]
        ):
            raise BenchmarkError(f"scenario receipt {scenario['id']} has invalid errors")
        for error in scenario["errors"]:
            failures[error] = failures.get(error, 0) + 1
        _number(scenario["latency_ms"], f"scenario receipt {scenario['id']} latency")
        expected_operations += scenario["expected_operations"]
        observed += scenario["observed_events"]
        duplicates += scenario["duplicates"]
        gaps += scenario["gaps"]
    resource = receipt.get("resource")
    if not isinstance(resource, dict):
        raise BenchmarkError("resource must be an object")
    for field in ("cpu_user_ms", "cpu_system_ms", "rss_peak_bytes", "disk_growth_bytes"):
        _number(resource.get(field), f"resource.{field}")
    energy = resource.get("energy_nj")
    if energy is not None:
        _number(energy, "resource.energy_nj")
    output_resource = {
        "cpu_user_ms": resource["cpu_user_ms"],
        "cpu_system_ms": resource["cpu_system_ms"],
        "rss_peak_bytes": resource["rss_peak_bytes"],
        "disk_growth_bytes": resource["disk_growth_bytes"],
        "energy_nj": energy,
    }
    if energy is None:
        reason = resource.get("energy_no_go_reason")
        if not isinstance(reason, str) or not reason.strip():
            raise BenchmarkError("missing explicit energy no-go reason")
        output_resource["energy_no_go_reason"] = reason
    return {
        "schema_version": 1,
        "source_revision": revision,
        "device": device,
        "scenario_count": len(scenarios),
        "latency_percentiles_ms": {
            "p50": _percentile(latencies, 50),
            "p95": _percentile(latencies, 95),
            "p99": _percentile(latencies, 99),
        },
        "coverage_classes": coverage,
        "duplicate_rate": duplicates / observed if observed else 0.0,
        "gap_rate": gaps / expected_operations if expected_operations else 0.0,
        "failure_counts": failures,
        "resource": output_resource,
        "limitations": [
            "FSEvents is a change-notification source and does not prove process causality.",
            "Results are comparable only after repeating the same workload on the same named device context.",
            "Energy is an explicit no-go when privileged power telemetry is unavailable.",
        ],
    }


def _run_native(profile: str) -> int:
    if platform.system() != "Darwin":
        print("filesystem-benchmark: native benchmark is a macOS-only no-go", file=sys.stderr)
        return 2
    command = ["cargo", "+1.88.0", "test", "--locked", "--test", "filesystem_benchmark"]
    if profile == "release":
        command.append("--release")
    command.extend([
        "macos::native_benchmark_runs_all_synthetic_workloads_and_emits_receipt",
        "--",
        "--exact",
        "--nocapture",
    ])
    try:
        revision = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except subprocess.SubprocessError as exc:
        print(f"filesystem-benchmark: cannot resolve source revision: {exc}", file=sys.stderr)
        return 1
    environment = dict(os.environ)
    environment["GHOSTRACE_BENCHMARK_REVISION"] = revision
    result = subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    sys.stderr.write(result.stderr)
    if result.returncode:
        return result.returncode
    prefix = "filesystem-benchmark-receipt="
    lines = [line[len(prefix):] for line in result.stdout.splitlines() if line.startswith(prefix)]
    if len(lines) != 1:
        print("filesystem-benchmark: native test did not emit one receipt", file=sys.stderr)
        return 1
    report = report_from_receipt(_load_json(lines[0], "native receipt"))
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    report = subparsers.add_parser("report")
    source = report.add_mutually_exclusive_group(required=True)
    source.add_argument("--stdin", action="store_true")
    source.add_argument("--input", type=Path)
    run = subparsers.add_parser("run")
    run.add_argument("--profile", choices=("debug", "release"), default="debug")
    args = parser.parse_args(argv)
    try:
        if args.command == "check":
            print(json.dumps(validate_corpus(_read_corpus()), indent=2, sort_keys=True))
            return 0
        if args.command == "report":
            raw = sys.stdin.read() if args.stdin else args.input.read_text(encoding="utf-8")
            print(json.dumps(report_from_receipt(_load_json(raw, "receipt")), indent=2, sort_keys=True))
            return 0
        return _run_native(args.profile)
    except (BenchmarkError, OSError, subprocess.SubprocessError) as exc:
        print(f"filesystem-benchmark: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
