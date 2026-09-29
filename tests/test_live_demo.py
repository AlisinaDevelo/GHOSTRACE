"""The guided demo script's contract (standard library only)."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "live-demo.sh"


class LiveDemoScriptTests(unittest.TestCase):
    def test_script_parses_and_refuses_without_a_built_binary(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
        result = subprocess.run(
            ["bash", str(SCRIPT), "/nonexistent/ghostrace"], capture_output=True, text=True
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("Build first", result.stderr)

    def test_script_isolates_itself_and_always_cleans_up(self) -> None:
        text = SCRIPT.read_text()
        self.assertIn("trap cleanup EXIT", text)
        self.assertIn("live forget", text)
        self.assertIn("GIT_CONFIG_GLOBAL=/dev/null", text)
        for forbidden in ("curl", "wget", "http://", "https://", "$HOME/"):
            self.assertNotIn(forbidden, text)

    def test_the_walkthrough_is_real_output_without_local_paths(self) -> None:
        demo = (ROOT / "docs" / "DEMO.md").read_text()
        self.assertIn("## 9. What GHOSTRACE did not observe", demo)
        self.assertIn("history_rewritten", demo)
        for leak in ("/Users/", "/private/var", "/var/folders"):
            self.assertNotIn(leak, demo)


if __name__ == "__main__":
    unittest.main()
