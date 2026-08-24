import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts" / "identity-audit.py"
MANIFEST = ROOT / "planning" / "identity-gate.json"


class IdentityAuditTests(unittest.TestCase):
    def run_checker(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), *args],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_manifest_passes_and_selects_collision_resistant_release_names(self):
        result = self.run_checker("check")
        self.assertEqual(result.returncode, 0, result.stderr or result.stdout)
        summary = json.loads(result.stdout)
        self.assertEqual(summary["collision_sources"], 8)
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        release = manifest["identifiers"]["release"]
        self.assertEqual(release["binary"], "ghostrace-journal")
        self.assertEqual(release["crate"], "ghostrace-journal")
        self.assertEqual(release["reverse_dns_identifier"], "com.alisinadevelo.ghostrace.journal")

    def test_legal_status_cannot_be_changed_to_cleared(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        manifest["legal_review"]["status"] = "cleared"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "identity-gate.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            result = self.run_checker("check", "--manifest", str(path))
        self.assertEqual(result.returncode, 2)
        self.assertIn("unresolved", json.loads(result.stdout)["error"])

    def test_provider_unavailability_is_retained(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        crates = next(item for item in manifest["collision_review"] if item["id"] == "crates-io")
        crates["result_state"] = "no_exact_record_observed"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "identity-gate.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            result = self.run_checker("check", "--manifest", str(path))
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(json.loads(result.stdout)["ok"], True)

    def test_bare_release_identifier_is_rejected(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        manifest["identifiers"]["release"]["binary"] = "ghostrace"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "identity-gate.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            result = self.run_checker("check", "--manifest", str(path))
        self.assertEqual(result.returncode, 2)
        self.assertIn("must be ghostrace-journal", json.loads(result.stdout)["error"])

    def test_domain_coverage_cannot_be_narrowed(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        domains = next(item for item in manifest["collision_review"] if item["id"] == "domains")
        domains["records"] = domains["records"][:-1]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "identity-gate.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            result = self.run_checker("check", "--manifest", str(path))
        self.assertEqual(result.returncode, 2)
        self.assertIn("all selected TLDs", json.loads(result.stdout)["error"])


if __name__ == "__main__":
    unittest.main()
