import json
import subprocess
import sys
import unittest
from pathlib import Path

from scripts import reproducibility


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
        self.assertEqual(report["fixtures"], 22)
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
        toolchains = reproducibility._active_ci_toolchain_values(workflow)
        self.assertNotIn("stable", toolchains)
        self.assertGreaterEqual(toolchains.count("1.88.0"), 3)

    def test_ci_toolchain_extraction_ignores_comments_and_run_blocks(self):
        workflow = "\n".join(
            (
                "jobs:",
                "  rust:",
                "    # toolchain: stable",
                "    steps:",
                "      - name: Decoy",
                "        run: |",
                "          toolchain: 1.88.0",
                "    strategy:",
                "      matrix:",
                "        include:",
                "          - toolchain: 1.88.0",
                "          - toolchain: ${{ matrix.toolchain }}",
            )
        )
        self.assertEqual(
            reproducibility._active_ci_toolchain_values(workflow),
            ["1.88.0", "${{ matrix.toolchain }}"],
        )

    def test_ci_runs_the_reproducibility_contract(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        manifest = json.loads((ROOT / "toolchain/manifest.json").read_text())
        self.assertEqual(
            reproducibility._check_ci_reproducibility_contract(workflow),
            manifest["github_actions"]["checkout"],
        )

    def test_ci_contract_rejects_non_executing_commands(self):
        checker = "python3 scripts/reproducibility.py check"
        tests = "python3 -m unittest discover -s tests -p 'test_reproducibility.py' -v"
        inactive_workflows = (
            "\n".join(
                (
                    "jobs:",
                    "  reproducibility:",
                    "    steps:",
                    f"      # run: {checker}",
                    f"      # run: {tests}",
                )
            ),
            "\n".join(
                (
                    "jobs:",
                    "  reproducibility:",
                    "    if: false",
                    "    steps:",
                    "      - name: Verify pinned inputs",
                    f"        run: {checker}",
                    "      - name: Test reproducibility contract",
                    f"        run: {tests}",
                )
            ),
        )
        for workflow in inactive_workflows:
            with self.subTest(workflow=workflow):
                with self.assertRaises(reproducibility.ReproducibilityError):
                    reproducibility._check_ci_reproducibility_contract(workflow)

    def test_ci_contract_rejects_commented_checkout_controls(self):
        checker = "python3 scripts/reproducibility.py check"
        tests = "python3 -m unittest discover -s tests -p 'test_reproducibility.py' -v"
        workflow = "\n".join(
            (
                "jobs:",
                "  reproducibility:",
                "    steps:",
                "      # - uses: actions/checkout@pinned",
                "      #   with:",
                "      #     persist-credentials: false",
                "      - name: Verify pinned inputs",
                f"        run: {checker}",
                "      - name: Test reproducibility contract",
                f"        run: {tests}",
            )
        )
        with self.assertRaises(reproducibility.ReproducibilityError):
            reproducibility._check_ci_reproducibility_contract(workflow)

    def test_ci_action_pin_extraction_ignores_comments_and_run_blocks(self):
        pinned = "actions-rust-lang/setup-rust-toolchain@pinned"
        floating = "actions-rust-lang/setup-rust-toolchain@main"
        workflow = "\n".join(
            (
                "jobs:",
                "  rust:",
                "    steps:",
                "      - name: Comment decoy",
                f"        # uses: {pinned}",
                "        run: |",
                f"          echo 'uses: {pinned}'",
                "      - name: Active action",
                f"        uses: {floating}",
            )
        )
        active = reproducibility._active_ci_action_refs(workflow)
        self.assertNotIn(pinned, active)
        self.assertEqual(active, {floating})

    def test_smoke_runner_is_executable_and_temporary(self):
        smoke = ROOT / "scripts/reproducibility-test.sh"
        indexed = subprocess.run(
            ["git", "ls-files", "--stage", "--", smoke.relative_to(ROOT).as_posix()],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(indexed.returncode, 0, indexed.stderr)
        git_mode, _, _ = indexed.stdout.partition(" ")
        self.assertEqual(git_mode, "100755")
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
