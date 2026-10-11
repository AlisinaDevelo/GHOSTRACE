"""Contract for the reproducible universal build script (standard library only)."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "build-universal.sh"


class BuildUniversalScriptTests(unittest.TestCase):
    def run_with_failed_build(
        self, directory: Path, *args: str, extra_env: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        tools = directory / "tools"
        tools.mkdir(exist_ok=True)
        cargo = tools / "cargo"
        cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$CARGO_TARGET_DIR" > "$BUILD_ATTEMPT"\nexit 17\n')
        cargo.chmod(0o755)
        return subprocess.run(
            ["bash", str(SCRIPT), *args], cwd=directory,
            env={**os.environ, "PATH": f"{tools}:{os.environ['PATH']}",
                 "BUILD_ATTEMPT": str(directory / "attempt"), **(extra_env or {})},
            capture_output=True, text=True, check=False,
        )

    def test_existing_output_is_preserved_before_any_build(self) -> None:
        for twice in (False, True):
            with self.subTest(twice=twice), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                output = directory / "unrelated-output"
                output.mkdir()
                sentinel = output / "keep.txt"
                sentinel.write_text("keep this file")
                args = ("--twice", str(output)) if twice else (str(output),)
                result = self.run_with_failed_build(directory, *args)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(sentinel.exists(), "preexisting output was deleted")
                self.assertEqual(sentinel.read_text(), "keep this file")
                self.assertFalse((directory / "attempt").exists())
                self.assertIn("already exists", result.stderr)

    def test_symlink_output_is_refused_before_any_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            output = directory / "output"
            output.symlink_to(directory / "missing", target_is_directory=True)
            result = self.run_with_failed_build(directory, str(output))
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue(output.is_symlink(), "output symlink was removed")
            self.assertFalse((directory / "attempt").exists())

    def test_relative_output_is_absolute_in_the_build_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = self.run_with_failed_build(directory, "output")
            self.assertEqual(result.returncode, 17, result.stderr)
            actual = (directory / "attempt").read_text().strip()
            self.assertTrue(Path(actual).is_absolute(), actual)
            self.assertEqual(Path(actual).resolve(), (directory / "output/target").resolve())

    def test_relative_output_ignores_cdpath(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = self.run_with_failed_build(directory, "output", extra_env={"CDPATH": "."})
            self.assertEqual(result.returncode, 17, result.stderr)
            actual = (directory / "attempt").read_text().strip()
            self.assertEqual(Path(actual).resolve(), (directory / "output/target").resolve())
            self.assertTrue((directory / "output/build-aarch64-apple-darwin.log").exists())

    def test_failed_twice_build_cleans_only_its_temporary_clone(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            tools = directory / "tools"
            tools.mkdir()
            mktemp = tools / "mktemp"
            mktemp.write_text(
                '#!/bin/sh\nmkdir "$CLONE_ROOT"\nprintf "%s\\n" "$CLONE_ROOT"\n'
            )
            mktemp.chmod(0o755)
            clone_root = directory / "synthetic-clone"
            result = self.run_with_failed_build(
                directory, "--twice", "output", extra_env={"CLONE_ROOT": str(clone_root)}
            )
            self.assertEqual(result.returncode, 17, result.stderr)
            self.assertFalse(clone_root.exists(), "failed build left its source clone")
            self.assertTrue((directory / "output/a/build-aarch64-apple-darwin.log").exists())

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
