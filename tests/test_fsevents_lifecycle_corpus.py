import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class FseventsLifecycleCorpusTests(unittest.TestCase):
    def run_checker(self):
        return subprocess.run(
            [sys.executable, str(ROOT / "scripts/fsevents-lifecycle-corpus.py"), "check"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_corpus_contract_covers_safe_and_guarded_lifecycle_scenarios(self):
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertTrue(report["ok"])
        self.assertEqual(report["schema_version"], 1)
        self.assertEqual(report["repeat_runs"], 32)
        self.assertEqual(report["scenario_count"], 9)
        self.assertEqual(report["native_device_scenarios"], [
            "bulk_checkout",
            "package_install",
            "rename_storm",
            "directory_deletion",
            "process_kill",
            "restart",
        ])
        self.assertEqual(report["guarded_no_go_scenarios"], ["sleep_wake", "logout", "volume_detach"])
        self.assertEqual(set(report["metrics"]), {
            "omission_rate",
            "duplicate_rate",
            "ordering_inversion_rate",
            "recovery_success_rate",
            "resource_peak_events",
        })

    def test_replay_distribution_is_byte_stable_and_bounded(self):
        first = self.run_checker()
        second = self.run_checker()
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(first.stdout, second.stdout)
        report = json.loads(first.stdout)
        self.assertLessEqual(report["replay_distribution"]["resource_peak_events"], 4096)
        self.assertGreaterEqual(report["replay_distribution"]["recovery_success_rate"], 1.0)
        self.assertIn("sleep_wake", report["replay_distribution"]["scenarios"])

    def test_fixture_is_public_safe_and_contains_no_path_or_content_fields(self):
        fixture = json.loads(
            (ROOT / "fixtures" / "fsevents-lifecycle-corpus-v1.json").read_text(encoding="utf-8")
        )
        def keys(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    yield key
                    yield from keys(child)
            elif isinstance(value, list):
                for child in value:
                    yield from keys(child)

        self.assertFalse({"path", "content", "plaintext", "url"}.intersection(keys(fixture)))
        self.assertTrue(fixture["privacy"]["synthetic_only"])
        self.assertFalse(fixture["privacy"]["user_data_included"])


if __name__ == "__main__":
    unittest.main()
