"""Contract for the reproducible universal build script (standard library only)."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "build-universal.sh"


class BuildUniversalScriptTests(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_inputs_are_pinned_and_paths_remapped_in_both_spellings(self) -> None:
        text = SCRIPT.read_text()
        for pinned in (
            "--locked",
            "MACOSX_DEPLOYMENT_TARGET",
            "SOURCE_DATE_EPOCH",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1",
            "CARGO_PROFILE_RELEASE_STRIP=symbols",
            "ZERO_AR_DATE=1",
        ):
            self.assertIn(pinned, text)
        # The resolved spelling is what the compiler records on macOS.
        self.assertIn("pwd -P", text)
        self.assertIn("--remap-path-prefix=$resolved_source=/ghostrace", text)
        self.assertIn("--remap-path-prefix=$resolved_home/.cargo=/cargo", text)
        self.assertIn("embedded_local_paths", text)

    def test_both_architectures_are_built_and_merged(self) -> None:
        text = SCRIPT.read_text()
        self.assertIn("aarch64-apple-darwin", text)
        self.assertIn("x86_64-apple-darwin", text)
        self.assertIn("lipo -create", text)


if __name__ == "__main__":
    unittest.main()
