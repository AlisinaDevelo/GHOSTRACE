"""Permission-manifest contract tests (standard library only)."""

from __future__ import annotations

import copy
import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("permissions", ROOT / "scripts" / "permissions.py")
permissions = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["permissions"] = permissions
SPEC.loader.exec_module(permissions)

CLI = "ghostrace-cli"
ADHOC = "CodeDirectory v=20400 size=43000 flags=0x20002(adhoc,linker-signed) hashes=1340+0"


class PermissionManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.document = permissions.load()
        self.cli = next(item for item in self.document["artifacts"] if item["id"] == CLI)

    def reapprove(self, document: dict) -> dict:
        document["review"]["approved_permission_digest"] = permissions.permission_digest(document)
        return document

    def test_checked_in_manifest_is_reviewed(self) -> None:
        report = permissions.validate(self.document)
        self.assertTrue(report["ok"])
        self.assertEqual(report["permission_digest"], permissions.permission_digest(self.document))

    def test_new_or_broadened_permission_fails_until_review_digest_is_updated(self) -> None:
        for mutate in (
            lambda artifact: artifact["entitlements"].update({"com.apple.security.files.user-selected.read-only": True}),
            lambda artifact: artifact["linked_libraries"].append("/usr/lib/libsqlite3.dylib"),
            lambda artifact: artifact["privacy_sensitive_apis"].append({"api": "accessibility", "framework": "ApplicationServices", "use": "x"}),
            lambda artifact: artifact.update({"filesystem_rights": "full disk access"}),
            lambda artifact: artifact.update({"sandboxed": True}),
            lambda artifact: artifact["helpers"].append("ghostrace-helper"),
        ):
            document = copy.deepcopy(self.document)
            mutate(next(item for item in document["artifacts"] if item["id"] == CLI))
            with self.assertRaisesRegex(permissions.ManifestError, "changed without review"):
                permissions.validate(document)
            permissions.validate(self.reapprove(document))

    def test_forbidden_entitlements_cannot_be_approved(self) -> None:
        for entitlement in (
            "com.apple.security.get-task-allow",
            "com.apple.security.cs.disable-library-validation",
            "com.apple.security.network.client",
        ):
            document = copy.deepcopy(self.document)
            next(item for item in document["artifacts"] if item["id"] == CLI)["entitlements"][entitlement] = True
            with self.assertRaisesRegex(permissions.ManifestError, "forbidden entitlement"):
                permissions.validate(self.reapprove(document))

    def test_network_capability_requires_a_new_review(self) -> None:
        document = copy.deepcopy(self.document)
        next(item for item in document["artifacts"] if item["id"] == CLI)["network_capability"] = "outbound"
        with self.assertRaisesRegex(permissions.ManifestError, "network capability"):
            permissions.validate(self.reapprove(document))

    def test_review_evidence_must_exist(self) -> None:
        document = copy.deepcopy(self.document)
        document["review"]["tests"] = ["tests/missing_permission_test.py"]
        with self.assertRaisesRegex(permissions.ManifestError, "missing"):
            permissions.validate(document)
        document = copy.deepcopy(self.document)
        document["review"]["migration"] = " "
        with self.assertRaisesRegex(permissions.ManifestError, "migration"):
            permissions.validate(document)

    def test_matching_artifact_has_no_drift(self) -> None:
        problems = permissions.compare_artifact(
            self.document, CLI, {}, sorted(self.cli["linked_libraries"]), ADHOC
        )
        self.assertEqual(problems, [])

    def test_artifact_drift_is_reported(self) -> None:
        cases = [
            ({"com.apple.security.get-task-allow": True}, self.cli["linked_libraries"], ADHOC, "forbidden entitlement"),
            ({"com.apple.security.cs.allow-jit": True}, self.cli["linked_libraries"], ADHOC, "forbidden entitlement"),
            ({"com.apple.security.device.camera": True}, self.cli["linked_libraries"], ADHOC, "not in the reviewed manifest"),
            ({}, self.cli["linked_libraries"] + ["/System/Library/Frameworks/CFNetwork.framework/Versions/A/CFNetwork"], ADHOC, "network library"),
            ({}, self.cli["linked_libraries"] + ["/usr/lib/libz.1.dylib"], ADHOC, "library not in the reviewed manifest"),
            ({}, self.cli["linked_libraries"], ADHOC.replace("adhoc,linker-signed", "adhoc,runtime"), "hardened runtime"),
            ({}, self.cli["linked_libraries"], ADHOC.replace("(adhoc,linker-signed)", "(none)"), "signature class"),
        ]
        for entitlements, libraries, flags, expected in cases:
            problems = permissions.compare_artifact(self.document, CLI, entitlements, sorted(libraries), flags)
            self.assertTrue(any(expected in problem for problem in problems), (expected, problems))

    def test_unknown_artifact_is_reported(self) -> None:
        problems = permissions.compare_artifact(self.document, "ghostrace-app", {}, [], ADHOC)
        self.assertEqual(problems, ["artifact ghostrace-app is not in the reviewed manifest"])

    @unittest.skipUnless(sys.platform == "darwin", "codesign and otool are macOS tools")
    def test_system_binary_signature_is_readable(self) -> None:
        flags = permissions.extract_signature_flags(Path("/bin/ls"))
        self.assertTrue(flags.startswith("CodeDirectory "))
        self.assertIn("/usr/lib/libSystem.B.dylib", permissions.extract_linked_libraries(Path("/bin/ls")))

    @unittest.skipUnless(sys.platform == "darwin", "codesign and otool are macOS tools")
    def test_resigned_binary_with_debug_entitlement_is_rejected(self) -> None:
        import plistlib
        import shutil
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "probe"
            shutil.copyfile("/usr/bin/true", binary)
            binary.chmod(0o755)
            entitlements = Path(directory) / "entitlements.plist"
            entitlements.write_bytes(plistlib.dumps({"com.apple.security.get-task-allow": True}))
            subprocess.run(
                ["codesign", "-f", "-s", "-", "--entitlements", str(entitlements), str(binary)],
                check=True,
                capture_output=True,
            )
            observed = permissions.extract_entitlements(binary)
            self.assertEqual(observed, {"com.apple.security.get-task-allow": True})
            problems = permissions.compare_artifact(
                self.document,
                CLI,
                observed,
                permissions.extract_linked_libraries(binary),
                permissions.extract_signature_flags(binary),
            )
            self.assertTrue(any("forbidden entitlement" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
