"""Signing cleanup with synthetic tools and a throwaway macOS keychain test."""

from __future__ import annotations

import json
import os
import platform
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "local-signing.sh"

# The external tools generate keys and modify a keychain, so failure tests
# replace only those boundaries. The script still creates and cleans real files.
SYNTHETIC_TOOL = r'''
import json, os, signal, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
work = Path(os.environ["SIGNING_TEST_WORK"])
failure = os.environ.get("SIGNING_TEST_FAILURE", "")
if name == "uname":
    print("Darwin")
elif name == "mktemp":
    work.mkdir(mode=0o700)
    print(work)
elif name == "openssl":
    for flag in ("-keyout", "-out"):
        if flag in args:
            Path(args[args.index(flag) + 1]).write_text("SYNTHETIC\n")
    if args[0] == "rand":
        print("synthetic-password")
    if failure == args[0] or (failure == "legacy" and "-legacy" in args):
        sys.exit(17)
elif name == "security":
    if args[0] == "find-certificate":
        if "-p" in args:
            print("SYNTHETIC CERTIFICATE")
            if failure == "certificate-output":
                sys.exit(17)
        elif os.environ["SIGNING_TEST_COMMAND"] == "create":
            sys.exit(1)
    elif args[0] == "import":
        Path(os.environ["SIGNING_TEST_MODES"]).write_text(json.dumps({
            p.name: p.stat().st_mode & 0o777 for p in work.iterdir()
        }))
        if os.environ.get("SIGNING_TEST_SIGNAL"):
            os.kill(os.getppid(), int(os.environ["SIGNING_TEST_SIGNAL"]))
    if failure == args[0]:
        sys.exit(17)
else:
    raise SystemExit("unexpected synthetic command")
'''


def run(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SCRIPT), *args], capture_output=True, text=True, env=env, check=False
    )


class LocalSigningScriptTests(unittest.TestCase):
    def synthetic_run(
        self, command: str, *, failure: str = "", termination: int | None = None
    ) -> tuple[subprocess.CompletedProcess[str], list[str], dict[str, int]]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tool_dir = root / "bin"
            tool_dir.mkdir()
            for name in ("uname", "mktemp", "openssl", "security"):
                tool = tool_dir / name
                tool.write_text(f"#!{sys.executable}\n{SYNTHETIC_TOOL}")
                tool.chmod(0o700)
            work = root / "work"
            modes_path = root / "modes.json"
            env = {
                **os.environ,
                "PATH": f"{tool_dir}{os.pathsep}{os.environ['PATH']}",
                "SIGNING_TEST_WORK": str(work),
                "SIGNING_TEST_MODES": str(modes_path),
                "SIGNING_TEST_COMMAND": command,
                "SIGNING_TEST_FAILURE": failure,
                "GHOSTRACE_SIGNING_KEYCHAIN": str(root / "synthetic.keychain"),
            }
            if termination is not None:
                env["SIGNING_TEST_SIGNAL"] = str(termination)
            result = subprocess.run(
                ["bash", str(SCRIPT), command], capture_output=True, text=True,
                env=env, check=False, timeout=10, umask=0,
            )
            remaining = (
                ["work", *(p.name for p in work.iterdir())] if work.exists() else []
            )
            modes = json.loads(modes_path.read_text()) if modes_path.exists() else {}
            return result, remaining, modes

    def test_create_removes_temporary_material_after_each_tool_failure(self) -> None:
        for failure in ("req", "rand", "pkcs12", "import"):
            with self.subTest(failure=failure):
                result, remaining, _ = self.synthetic_run("create", failure=failure)
                self.assertEqual(result.returncode, 17, result.stderr)
                self.assertEqual(remaining, [])

    def test_certificate_output_failure_cleans_up_trust_and_remove(self) -> None:
        for command in ("trust", "remove"):
            with self.subTest(command=command):
                result, remaining, _ = self.synthetic_run(
                    command, failure="certificate-output"
                )
                self.assertEqual(result.returncode, 17, result.stderr)
                self.assertEqual(remaining, [])

    def test_keychain_mutation_failure_cleans_up_trust_and_remove(self) -> None:
        for command, failure in (("trust", "add-trusted-cert"), ("remove", "delete-identity")):
            with self.subTest(command=command):
                result, remaining, _ = self.synthetic_run(command, failure=failure)
                self.assertEqual(result.returncode, 17, result.stderr)
                self.assertEqual(remaining, [])

    def test_temporary_files_are_private_even_with_permissive_caller_umask(self) -> None:
        result, remaining, modes = self.synthetic_run("create")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(set(modes), {"cert.cnf", "cert.pem", "identity.p12", "key.pem"})
        self.assertTrue(all(mode == 0o600 for mode in modes.values()), modes)
        self.assertEqual(remaining, [])

    def test_legacy_pkcs12_fallback_still_succeeds_and_cleans_up(self) -> None:
        result, remaining, _ = self.synthetic_run("create", failure="legacy")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(remaining, [])

    def test_termination_signals_remove_temporary_key_material(self) -> None:
        for termination in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
            with self.subTest(termination=termination):
                result, remaining, _ = self.synthetic_run("create", termination=termination)
                self.assertEqual(result.returncode, 128 + termination, result.stderr)
                self.assertEqual(remaining, [])

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
