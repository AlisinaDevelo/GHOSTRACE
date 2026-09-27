"""Compatibility-matrix contract tests (standard library only)."""

from __future__ import annotations

import copy
import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("compatibility", ROOT / "scripts" / "compatibility.py")
compatibility = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["compatibility"] = compatibility
SPEC.loader.exec_module(compatibility)


class CompatibilityMatrixTests(unittest.TestCase):
    def setUp(self) -> None:
        self.document = compatibility.load()

    def entry(self, document: dict, format_id: str) -> dict:
        return next(item for item in document["formats"] if item["id"] == format_id)

    def test_checked_in_matrix_is_complete_and_proven(self) -> None:
        report = compatibility.validate(self.document)
        self.assertTrue(report["ok"])
        self.assertGreaterEqual(report["formats"], 5)

    def test_every_required_case_must_be_present(self) -> None:
        document = copy.deepcopy(self.document)
        del self.entry(document, "export-stream")["cases"]["mixed"]
        with self.assertRaisesRegex(compatibility.MatrixError, "missing cases"):
            compatibility.validate(document)

    def test_unsafe_cases_cannot_be_accepted(self) -> None:
        for format_id, case in (
            ("event-envelope", "forward"),
            ("event-envelope", "unknown_field"),
            ("export-stream", "mixed"),
            ("journal-database", "partially_migrated"),
            ("journal-database", "downgrade"),
        ):
            document = copy.deepcopy(self.document)
            self.entry(document, format_id)["cases"][case]["outcome"] = "accept"
            with self.assertRaisesRegex(compatibility.MatrixError, "must be refused"):
                compatibility.validate(document)

    def test_a_proving_test_that_does_not_exist_fails(self) -> None:
        document = copy.deepcopy(self.document)
        self.entry(document, "event-envelope")["cases"]["forward"]["test"] = (
            "tests/compatibility_matrix.rs::no_such_test"
        )
        with self.assertRaisesRegex(compatibility.MatrixError, "does not exist"):
            compatibility.validate(document)

    def test_deprecation_requires_the_full_record(self) -> None:
        document = copy.deepcopy(self.document)
        entry = self.entry(document, "shell-execution-metadata")
        entry["status"] = "deprecated"
        with self.assertRaisesRegex(compatibility.MatrixError, "deprecation record"):
            compatibility.validate(document)
        entry["deprecation"] = {
            "announced_in": "0.1.0",
            "removal_not_before": "0.3.0",
            "migration_tool": "scripts/no-such-migrator.py",
            "rollback_evidence": "docs/evidence",
            "release_note": "Shell metadata v1 is replaced by v2.",
        }
        with self.assertRaisesRegex(compatibility.MatrixError, "migration_tool does not exist"):
            compatibility.validate(document)
        entry["deprecation"]["migration_tool"] = "scripts/compatibility.py"
        compatibility.validate(document)

    def test_retiring_a_version_requires_deprecation_and_removal_from_reads(self) -> None:
        document = copy.deepcopy(self.document)
        record = {
            "announced_in": "0.1.0",
            "removal_not_before": "0.3.0",
            "migration_tool": "scripts/compatibility.py",
            "rollback_evidence": "docs/evidence",
            "release_note": "Journal schema 1 upgrades are no longer supported.",
        }
        document["retired"] = [{"id": "journal-database", "version": 1, "deprecation": record}]
        with self.assertRaisesRegex(compatibility.MatrixError, "still listed as readable"):
            compatibility.validate(document)
        self.entry(document, "journal-database")["supported_read"].remove(1)
        compatibility.validate(document)
        document["retired"][0]["deprecation"] = {}
        with self.assertRaisesRegex(compatibility.MatrixError, "deprecation record"):
            compatibility.validate(document)

    def test_writers_emit_the_current_version_and_goldens_exist(self) -> None:
        document = copy.deepcopy(self.document)
        self.entry(document, "git-snapshot-metadata")["write_version"] = 0
        with self.assertRaisesRegex(compatibility.MatrixError, "current version"):
            compatibility.validate(document)
        document = copy.deepcopy(self.document)
        self.entry(document, "git-snapshot-metadata")["golden"] = "fixtures/missing.json"
        with self.assertRaisesRegex(compatibility.MatrixError, "golden"):
            compatibility.validate(document)


if __name__ == "__main__":
    unittest.main()
