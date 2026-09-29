"""Local signing identity script, against a throwaway keychain (macOS only)."""

from __future__ import annotations

import os
import platform
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "local-signing.sh"


def run(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SCRIPT), *args], capture_output=True, text=True, env=env, check=False
    )


class LocalSigningScriptTests(unittest.TestCase):
    def test_help_lists_every_command(self) -> None:
        result = run("help")
        self.assertEqual(result.returncode, 0)
        for command in ("create", "trust", "sign", "status", "remove"):
            self.assertIn(command, result.stdout)
        self.assertEqual(run("bogus").returncode, 2)

    @unittest.skipUnless(platform.system() == "Darwin", "macOS keychain")
    def test_create_is_idempotent_keeps_the_key_non_extractable_and_remove_cleans_up(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            keychain = str(Path(directory) / "signing-test.keychain-db")
            subprocess.run(["security", "create-keychain", "-p", "test", keychain], check=True)
            try:
                subprocess.run(["security", "unlock-keychain", "-p", "test", keychain], check=True)
                env = {**os.environ, "GHOSTRACE_SIGNING_KEYCHAIN": keychain}
                self.assertIn("No ", run("status", env=env).stdout)
                self.assertEqual(run("create", env=env).returncode, 0)
                self.assertIn("already exists", run("create", env=env).stdout)
                self.assertIn("not trusted", run("status", env=env).stdout)
                # The private key cannot be exported again.
                exported = subprocess.run(
                    ["security", "export", "-k", keychain, "-t", "privKeys", "-f", "pkcs12",
                     "-P", "x"],
                    capture_output=True, text=True, check=False, timeout=60,
                )
                self.assertNotEqual(exported.returncode, 0)
                self.assertIn("cannot be retrieved", exported.stderr)
                # Nothing but the keychain itself was left in the directory.
                self.assertEqual(
                    [p.name for p in Path(directory).iterdir() if not p.name.startswith(".")],
                    ["signing-test.keychain-db"],
                )
                self.assertEqual(run("remove", env=env).returncode, 0)
                self.assertIn("No ", run("status", env=env).stdout)
            finally:
                subprocess.run(["security", "delete-keychain", keychain], check=False)


if __name__ == "__main__":
    unittest.main()
