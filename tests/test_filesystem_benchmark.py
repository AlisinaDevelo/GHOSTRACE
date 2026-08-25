#!/usr/bin/env python3
"""Contract tests for the reproducible filesystem benchmark corpus."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "filesystem-benchmark.py"
CORPUS = ROOT / "fixtures" / "filesystem-benchmark-corpus-v1.json"


class FilesystemBenchmarkContractTests(unittest.TestCase):
    def test_check_validates_the_versioned_synthetic_corpus(self) -> None:
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "check"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertTrue(report["ok"])
        self.assertEqual(report["scenario_count"], 8)
        self.assertEqual(report["repeat_runs"], 3)
        self.assertEqual(report["metrics"][-1], "disk_growth_bytes")

    def test_report_rejects_paths_and_content_like_fields(self) -> None:
        receipt = {
            "schema_version": 1,
            "source_revision": "0" * 40,
            "device": {"model": "fixture", "os": "fixture", "arch": "arm64", "toolchain": "fixture"},
            "latency_samples_ms": [1.0],
            "scenarios": [],
            "resource": {
                "cpu_user_ms": 0,
                "cpu_system_ms": 0,
                "rss_peak_bytes": 0,
                "disk_growth_bytes": 0,
                "energy_nj": None,
            },
            "path": "/private/user-data",
        }
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "report", "--stdin"],
            cwd=ROOT,
            input=json.dumps(receipt),
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("forbidden", result.stderr.lower())

    def test_report_calculates_percentiles_and_preserves_energy_no_go(self) -> None:
        receipt = {
            "schema_version": 1,
            "source_revision": "0" * 40,
            "device": {"model": "fixture", "os": "fixture", "arch": "arm64", "toolchain": "fixture"},
            "latency_samples_ms": [1.0, 2.0, 10.0],
            "scenarios": [
                {
                    "id": "small_tree",
                    "coverage": {"direct": 2, "contextual": 1, "inferred": 0, "unknown": 0},
                    "expected_operations": 3,
                    "observed_events": 3,
                    "duplicates": 0,
                    "gaps": 0,
                    "errors": [],
                    "latency_ms": 2.0,
                }
            ],
            "resource": {
                "cpu_user_ms": 1,
                "cpu_system_ms": 2,
                "rss_peak_bytes": 4096,
                "disk_growth_bytes": 512,
                "energy_nj": None,
                "energy_no_go_reason": "powermetrics requires interactive authorization",
            },
        }
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "report", "--stdin"],
            cwd=ROOT,
            input=json.dumps(receipt),
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["latency_percentiles_ms"], {"p50": 2.0, "p95": 10.0, "p99": 10.0})
        self.assertEqual(report["coverage_classes"], {"contextual": 1, "direct": 2, "inferred": 0, "unknown": 0})
        self.assertEqual(report["resource"]["energy_nj"], None)
        self.assertIn("energy_no_go_reason", report["resource"])


if __name__ == "__main__":
    unittest.main()
