import json
import stat
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class ReproducibilityContractTests(unittest.TestCase):
    def run_script(self, relative, *args):
        return subprocess.run(
            [sys.executable, str(ROOT / relative), *args],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_fixture_manifest_is_checked_and_synthetic(self):
        result = self.run_script("scripts/fixture-manifest.py", "check")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["fixtures"], 4)
        self.assertEqual(report["generator_version"], "ghostrace-fixture-manifest-v1")
        self.assertEqual(report["seed"], "ghostrace-fixture-seed-v1")
        manifest = json.loads((ROOT / "fixtures/manifest.json").read_text())
        self.assertEqual(
            manifest["privacy"],
            {"synthetic_only": True, "user_data_included": False, "network_required": False},
        )

    def test_toolchain_manifest_is_checked_without_network(self):
        result = self.run_script("scripts/reproducibility.py", "check")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["rust_channel"], "1.88.0")
        self.assertTrue(report["ok"])

    def test_ci_uses_the_pinned_toolchain(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertNotIn("toolchain: stable", workflow)
        self.assertGreaterEqual(workflow.count("toolchain: 1.88.0"), 3)

    def test_smoke_runner_is_executable_and_temporary(self):
        smoke = ROOT / "scripts/reproducibility-test.sh"
        self.assertTrue(stat.S_IMODE(smoke.stat().st_mode) & stat.S_IXUSR)
        self.assertIn('mktemp -d', smoke.read_text())
        self.assertIn('trap', smoke.read_text())

    def test_fixture_paths_are_repository_relative(self):
        manifest = json.loads((ROOT / "fixtures/manifest.json").read_text())
        for fixture in manifest["fixtures"]:
            path = Path(fixture["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertTrue((ROOT / path).is_file())


if __name__ == "__main__":
    unittest.main()
