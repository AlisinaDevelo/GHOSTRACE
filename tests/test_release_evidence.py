import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts" / "release-evidence.py"
REGISTER = ROOT / "planning" / "release-evidence-register.json"


class ReleaseEvidenceRegisterTests(unittest.TestCase):
    def run_checker(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), *args],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_register_covers_all_milestones_and_all_four_states(self):
        result = self.run_checker("check")
        self.assertEqual(result.returncode, 0, result.stderr or result.stdout)
        summary = json.loads(result.stdout)
        self.assertEqual(summary["milestones"], 12)
        self.assertGreaterEqual(summary["measures"], 30)
        register = json.loads(REGISTER.read_text(encoding="utf-8"))
        self.assertEqual(
            set(register["states"]),
            {"planned", "observed", "inferred", "unavailable"},
        )

    def test_gate_blocks_planned_evidence(self):
        result = self.run_checker("gate", "--milestone", "M0", "--as-of", "2026-08-24")
        self.assertEqual(result.returncode, 1, result.stdout)
        gate = json.loads(result.stdout)
        self.assertFalse(gate["allowed"])
        self.assertTrue(all(item["code"] == "state_planned" for item in gate["blockers"]))

    def test_inferred_and_unavailable_evidence_also_block(self):
        for state in ("inferred", "unavailable"):
            register = json.loads(REGISTER.read_text(encoding="utf-8"))
            register["milestones"][0]["exit_measures"][0]["evidence"]["state"] = state
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "register.json"
                path.write_text(json.dumps(register), encoding="utf-8")
                result = self.run_checker(
                    "gate",
                    "--register",
                    str(path),
                    "--milestone",
                    "M0",
                    "--as-of",
                    "2026-08-24",
                )
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertEqual(json.loads(result.stdout)["blockers"][0]["code"], f"state_{state}")

    def test_gate_rejects_stale_observed_artifact(self):
        register = json.loads(REGISTER.read_text(encoding="utf-8"))
        measure = register["milestones"][0]["exit_measures"][0]
        measure["artifact"] = "docs/evidence/0043-privacy-regression.md"
        measure["evidence"] = {
            "state": "observed",
            "artifact": "docs/evidence/0043-privacy-regression.md",
            "observed_at": "2025-01-01",
            "scope": measure["scope"],
            "notes": "fixture evidence is intentionally stale for this test",
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "register.json"
            path.write_text(json.dumps(register), encoding="utf-8")
            result = self.run_checker(
                "gate",
                "--register",
                str(path),
                "--milestone",
                "M0",
                "--as-of",
                "2026-08-24",
            )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("stale", {item["code"] for item in json.loads(result.stdout)["blockers"]})

    def test_narrow_scope_is_rejected_before_gate_evaluation(self):
        register = json.loads(REGISTER.read_text(encoding="utf-8"))
        measure = register["milestones"][0]["exit_measures"][0]
        measure["evidence"]["scope"] = ["M0"]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "register.json"
            path.write_text(json.dumps(register), encoding="utf-8")
            result = self.run_checker(
                "gate",
                "--register",
                str(path),
                "--milestone",
                "M0",
                "--as-of",
                "2026-08-24",
            )
        self.assertEqual(result.returncode, 2, result.stdout)
        self.assertIn("narrower", json.loads(result.stdout)["error"])


if __name__ == "__main__":
    unittest.main()
