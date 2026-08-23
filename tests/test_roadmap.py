import copy
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("roadmap", ROOT / "scripts" / "roadmap.py")
assert SPEC is not None and SPEC.loader is not None
roadmap = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = roadmap
SPEC.loader.exec_module(roadmap)


class FakeGitHub:
    responses = {
        "labels": [],
        "milestones?state=all": [],
        "issues?state=all": [],
    }
    requests = []

    def __init__(self, repository):
        self.repository = repository

    def paginated(self, path):
        path = path.split("&per_page=", 1)[0]
        for suffix, value in self.responses.items():
            if path.endswith(suffix):
                return copy.deepcopy(value)
        raise AssertionError(path)

    def request(self, method, path, payload=None):
        self.requests.append((method, path, payload))
        raise AssertionError("a blocker must prevent writes")


def tiny_program(labels=None):
    return {
        "schema_version": 1,
        "project": "GHOSTRACE",
        "repository": "AlisinaDevelo/GHOSTRACE",
        "owner": "AlisinaDevelo",
        "horizon": {
            "starts_on": "2026-08-23",
            "ends_on": "2026-12-31",
            "minimum_task_count": 1,
        },
        "milestones": [
            {
                "title": "M0",
                "due_on": "2026-12-31T23:59:59Z",
                "description": "test milestone",
            }
        ],
        "labels": labels
        or [
            {"name": "epic:test", "color": "5319e7", "description": "test"},
            {"name": "type:test", "color": "0e8a16", "description": "test"},
            {"name": "priority:p0", "color": "b60205", "description": "test"},
            {"name": "status:ready", "color": "0e8a16", "description": "test"},
        ],
    }


def tiny_task(status="ready"):
    return roadmap.Task(
        task_id="0001",
        title="Test task",
        status=status,
        agent="maintainer",
        model="human",
        release="M0",
        parent=None,
        depends_on=(),
        workstream="test",
        kind="test",
        priority="p0",
        risks=(),
        platform="any",
        goal="A test goal",
        criteria=((False, "one"), (False, "two")),
        context="A test context",
        notes="Test notes",
        path=ROOT / ".forge" / "tasks" / "0001-test-task.md",
        source_sha256="0" * 64,
    )


def fake_issue(
    number,
    body,
    *,
    labels=None,
    assignees=None,
    milestone=None,
):
    return {
        "number": number,
        "title": f"Issue {number}",
        "state": "open",
        "updated_at": "2026-08-23T00:00:00Z",
        "body": body,
        "labels": labels or [],
        "assignees": assignees or [],
        "milestone": milestone,
    }


class RoadmapTests(unittest.TestCase):
    def test_forge_sync_hash_matches_non_ascii_canonicalization(self):
        task = replace(tiny_task(), notes="Planned in 2026–2031.")
        self.assertEqual(
            roadmap.forge_task_sync_hash(task),
            "6047b2d52ff62f1c40cd1ca8019ddd738c8a7ad998d80a9d531e820ddd664d3b",
        )

    def test_checkbox_state_is_canonical_for_task_status(self):
        task = replace(
            tiny_task(),
            criteria=((True, "one"), (False, "two")),
        )
        with self.assertRaisesRegex(
            roadmap.RoadmapError,
            "non-done task has checked acceptance criteria",
        ):
            roadmap.validate(tiny_program(), [task])

    def test_task_snapshot_digest_is_not_re_read_during_planning(self):
        program = tiny_program()
        parsed = tiny_task()
        current = replace(parsed, source_sha256="1" * 64)
        with mock.patch.object(
            roadmap, "load_program", return_value=program
        ), mock.patch.object(roadmap, "load_tasks", return_value=[current]):
            with self.assertRaisesRegex(roadmap.RoadmapError, "task tree changed"):
                roadmap.verify_local_snapshot_current(program, [parsed])

    def test_live_forge_parity_is_rechecked_with_a_bounded_schema(self):
        clean = SimpleNamespace(
            returncode=0,
            stderr="",
            stdout=json.dumps({"authority": "local", "operations": []}),
        )
        with mock.patch.object(roadmap.subprocess, "run", return_value=clean) as run:
            digest = roadmap.verify_live_forge_parity(
                ROOT / "scripts" / "roadmap.py",
                "AlisinaDevelo/GHOSTRACE",
            )
        self.assertRegex(digest, r"^[0-9a-f]{64}$")
        self.assertIn("plan", run.call_args.args[0])

        dirty = SimpleNamespace(
            returncode=0,
            stderr="",
            stdout=json.dumps(
                {"authority": "local", "operations": [{"action": "update_issue"}]}
            ),
        )
        with mock.patch.object(roadmap.subprocess, "run", return_value=dirty):
            with self.assertRaisesRegex(roadmap.RoadmapError, "pending operation"):
                roadmap.verify_live_forge_parity(
                    ROOT / "scripts" / "roadmap.py",
                    "AlisinaDevelo/GHOSTRACE",
                )

    def test_apply_does_not_write_when_preflight_has_blocker(self):
        program = roadmap.load_program()
        tasks = roadmap.load_tasks()
        FakeGitHub.responses = {
            "labels": [],
            "milestones?state=all": [],
            "issues?state=all": [],
        }
        FakeGitHub.requests = []
        with mock.patch.object(roadmap, "GitHub", FakeGitHub), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={}
        ):
            forge_digest = "0" * 64
            operations = roadmap.metadata_plan(
                program, tasks, program["repository"], forge_digest
            )
            digest = roadmap.plan_digest(
                program["repository"], operations, forge_digest
            )
            with self.assertRaises(roadmap.RoadmapError):
                roadmap.apply_metadata(
                    program,
                    tasks,
                    program["repository"],
                    digest,
                    forge_digest,
                    ROOT / "scripts" / "roadmap.py",
                )
        self.assertEqual(FakeGitHub.requests, [])

    def test_apply_executes_only_the_inspected_plan(self):
        program = tiny_program()
        task = tiny_task()
        forge_digest = "0" * 64
        operations = [
            {
                "action": "create_label",
                "blocking": False,
                "name": "epic:test",
                "payload": program["labels"][0],
            }
        ]
        digest = roadmap.plan_digest(
            program["repository"], operations, forge_digest
        )

        class RecordingGitHub:
            requests = []

            def __init__(self, repository):
                self.repository = repository

            def request(self, method, path, payload=None):
                self.requests.append((method, path, payload))
                return {}

        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap,
            "metadata_plan",
            side_effect=[operations, operations, []],
        ), mock.patch.object(roadmap, "GitHub", RecordingGitHub), mock.patch.object(
            roadmap.time, "sleep"
        ) as sleep, mock.patch.object(
            roadmap, "verify_local_snapshot_current"
        ), mock.patch.object(
            roadmap, "verify_live_forge_parity", return_value="f" * 64
        ):
            result = roadmap.apply_metadata(
                program,
                [task],
                program["repository"],
                digest,
                forge_digest,
                ROOT / "scripts" / "roadmap.py",
            )
        self.assertEqual(len(RecordingGitHub.requests), 1)
        self.assertEqual(result["counts"], {"create_label": 1})
        self.assertEqual(result["remaining_operations"], 0)
        sleep.assert_called_once_with(roadmap.MUTATION_INTERVAL_SECONDS)

    def test_marker_conflict_is_a_preflight_blocker(self):
        program = tiny_program()
        task = tiny_task()
        FakeGitHub.responses = {
            "labels": [],
            "milestones?state=all": [{"number": 7, "title": "M0"}],
            "issues?state=all": [
                fake_issue(
                    1,
                    "<!-- forge-task:v1 id=0002 -->",
                    milestone={"number": 7, "title": "M0"},
                )
            ],
        }
        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={"0001": 1}
        ), mock.patch.object(roadmap, "GitHub", FakeGitHub):
            operations = roadmap.metadata_plan(program, [task], program["repository"])
        conflicts = [item for item in operations if item["action"] == "marker_conflict"]
        self.assertEqual(len(conflicts), 1)
        self.assertTrue(conflicts[0]["blocking"])

    def test_extra_malformed_marker_declaration_is_a_blocker(self):
        program = tiny_program()
        task = tiny_task()
        body = (
            "<!-- forge-task:v1 id=0001 "
            f"sync={roadmap.forge_task_sync_hash(task)} -->\n"
            "<!-- forge-task:v1 id=0001 sync=invalid -->"
        )
        FakeGitHub.responses = {
            "labels": program["labels"],
            "milestones?state=all": [{"number": 7, "title": "M0"}],
            "issues?state=all": [
                fake_issue(
                    1,
                    body,
                    milestone={"number": 7, "title": "M0"},
                )
            ],
        }
        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={"0001": 1}
        ), mock.patch.object(roadmap, "GitHub", FakeGitHub):
            operations = roadmap.metadata_plan(
                program,
                [task],
                program["repository"],
                "0" * 64,
            )
        invalid = [item for item in operations if item["action"] == "invalid_marker"]
        self.assertEqual(len(invalid), 1)
        self.assertEqual(invalid[0]["declarations"], 2)
        self.assertEqual(invalid[0]["parsed"], 1)

    def test_exact_program_labels_and_unmanaged_assignee_are_preserved(self):
        program = tiny_program()
        task = tiny_task()
        FakeGitHub.responses = {
            "labels": program["labels"],
            "milestones?state=all": [{"number": 7, "title": "M0"}],
            "issues?state=all": [
                fake_issue(
                    1,
                    (
                        "<!-- forge-task:v1 id=0001 "
                        f"sync={roadmap.forge_task_sync_hash(task)} -->"
                    ),
                    labels=[{"name": "human:keep"}, {"name": "epic:unmanaged"}],
                    assignees=[{"login": "human-owner"}],
                    milestone={"number": 7, "title": "M0"},
                )
            ],
        }
        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={"0001": 1}
        ), mock.patch.object(roadmap, "GitHub", FakeGitHub):
            operations = roadmap.metadata_plan(program, [task], program["repository"])
        self.assertTrue(
            any(item["action"] == "add_owner_assignee" for item in operations)
        )
        added_labels = {
            item["name"]
            for item in operations
            if item["action"] == "add_issue_label"
        }
        self.assertEqual(added_labels, set(roadmap.desired_labels(task)))
        removed_labels = {
            item["name"]
            for item in operations
            if item["action"] == "remove_issue_label"
        }
        self.assertNotIn("human:keep", removed_labels)
        self.assertNotIn("epic:unmanaged", removed_labels)

    def test_sync_mapping_rejects_repository_and_duplicate_issue_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "github-sync.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "repository": "someone/else",
                        "tasks": {"0001": {"issue": 1}},
                    }
                ),
                encoding="utf-8",
            )
            with mock.patch.object(roadmap, "SYNC_STATE_PATH", path):
                with self.assertRaises(roadmap.RoadmapError):
                    roadmap.load_sync_mapping("AlisinaDevelo/GHOSTRACE", {"0001"})

            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "repository": "AlisinaDevelo/GHOSTRACE",
                        "tasks": {
                            "0001": {"issue": 1},
                            "0002": {"issue": 1},
                        },
                    }
                ),
                encoding="utf-8",
            )
            with mock.patch.object(roadmap, "SYNC_STATE_PATH", path):
                with self.assertRaises(roadmap.RoadmapError):
                    roadmap.load_sync_mapping("AlisinaDevelo/GHOSTRACE", {"0001", "0002"})

    def test_task_discovery_rejects_symlink_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            task_dir = Path(directory) / "tasks"
            task_dir.mkdir()
            (task_dir / "README.md").write_text("index\n", encoding="utf-8")
            target = task_dir / "0001-real.md"
            target.write_text("not used\n", encoding="utf-8")
            os.symlink(target, task_dir / "0000-link.md")
            with mock.patch.object(roadmap, "TASKS_DIR", task_dir):
                with self.assertRaisesRegex(roadmap.RoadmapError, "must not be a symlink"):
                    roadmap.load_tasks()

    def test_plan_digest_is_order_independent_and_github_retries_after_status(self):
        operations = [{"action": "b", "value": 2}, {"value": 1, "action": "a"}]
        self.assertEqual(
            roadmap.plan_digest("AlisinaDevelo/GHOSTRACE", operations),
            roadmap.plan_digest("AlisinaDevelo/GHOSTRACE", list(reversed(operations))),
        )
        self.assertNotEqual(
            roadmap.plan_digest("AlisinaDevelo/GHOSTRACE", operations, "0" * 64),
            roadmap.plan_digest("AlisinaDevelo/GHOSTRACE", operations, "1" * 64),
        )
        responses = [
            SimpleNamespace(returncode=1, stderr="HTTP 429", stdout=""),
            SimpleNamespace(returncode=0, stderr="", stdout="{}"),
        ]
        with mock.patch.object(roadmap.subprocess, "run", side_effect=responses), mock.patch.object(
            roadmap.time, "sleep"
        ) as sleep:
            self.assertEqual(roadmap.GitHub("AlisinaDevelo/GHOSTRACE").request("GET", "repos/test"), {})
        sleep.assert_called_once_with(1.0)

    def test_clean_forge_plan_is_required_and_digest_is_canonical(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "forge-plan.json"
            path.write_text(
                json.dumps(
                    {
                        "operations": [],
                        "authority": "local",
                        "repository": "AlisinaDevelo/GHOSTRACE",
                        "task_tree_digest": "2" * 64,
                    }
                ),
                encoding="utf-8",
            )
            digest = roadmap.load_forge_parity_plan(
                path, "AlisinaDevelo/GHOSTRACE", "2" * 64
            )
            self.assertRegex(digest, r"^[0-9a-f]{64}$")
            with self.assertRaisesRegex(roadmap.RoadmapError, "does not match"):
                roadmap.load_forge_parity_plan(
                    path, "AlisinaDevelo/OTHER", "2" * 64
                )
            with self.assertRaisesRegex(roadmap.RoadmapError, "task tree"):
                roadmap.load_forge_parity_plan(
                    path, "AlisinaDevelo/GHOSTRACE", "3" * 64
                )
            path.write_text(
                json.dumps(
                    {
                        "authority": "local",
                        "repository": "AlisinaDevelo/GHOSTRACE",
                        "task_tree_digest": "2" * 64,
                        "operations": [{"action": "update_issue"}],
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(roadmap.RoadmapError, "pending operation"):
                roadmap.load_forge_parity_plan(
                    path, "AlisinaDevelo/GHOSTRACE", "2" * 64
                )

    def test_stale_issue_sync_hash_is_a_blocker(self):
        program = tiny_program()
        task = tiny_task()
        FakeGitHub.responses = {
            "labels": program["labels"],
            "milestones?state=all": [{"number": 7, "title": "M0"}],
            "issues?state=all": [
                fake_issue(
                    1,
                    f"<!-- forge-task:v1 id=0001 sync={'a' * 64} -->",
                    milestone={"number": 7, "title": "M0"},
                )
            ],
        }
        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={"0001": 1}
        ), mock.patch.object(roadmap, "GitHub", FakeGitHub):
            operations = roadmap.metadata_plan(
                program, [task], program["repository"], "0" * 64
            )
        conflicts = [
            item for item in operations if item["action"] == "marker_sync_conflict"
        ]
        self.assertEqual(len(conflicts), 1)
        self.assertTrue(conflicts[0]["blocking"])

    def test_issue_metadata_mutations_are_field_scoped(self):
        program = tiny_program()
        task = tiny_task()
        forge_digest = "0" * 64
        operations = [
            {
                "action": "add_issue_label",
                "blocking": False,
                "task": "0001",
                "number": 1,
                "name": "status:ready",
            },
            {
                "action": "add_owner_assignee",
                "blocking": False,
                "task": "0001",
                "number": 1,
                "owner": program["owner"],
            },
            {
                "action": "set_issue_milestone",
                "blocking": False,
                "task": "0001",
                "number": 1,
                "milestone": 7,
            },
        ]
        digest = roadmap.plan_digest(
            program["repository"], operations, forge_digest
        )
        requests = []

        class RecordingGitHub:
            def __init__(self, repository):
                self.repository = repository

            def request(self, method, path, payload=None):
                requests.append((method, path, payload))
                return {}

        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "metadata_plan", side_effect=[operations, operations, []]
        ), mock.patch.object(roadmap, "GitHub", RecordingGitHub), mock.patch.object(
            roadmap, "verify_local_snapshot_current"
        ), mock.patch.object(
            roadmap, "verify_live_forge_parity", return_value="f" * 64
        ), mock.patch.object(roadmap.time, "sleep"):
            roadmap.apply_metadata(
                program,
                [task],
                program["repository"],
                digest,
                forge_digest,
                ROOT / "scripts" / "roadmap.py",
            )
        self.assertEqual(
            requests,
            [
                (
                    "POST",
                    "repos/AlisinaDevelo/GHOSTRACE/issues/1/labels",
                    {"labels": ["status:ready"]},
                ),
                (
                    "POST",
                    "repos/AlisinaDevelo/GHOSTRACE/issues/1/assignees",
                    {"assignees": [program["owner"]]},
                ),
                (
                    "PATCH",
                    "repos/AlisinaDevelo/GHOSTRACE/issues/1",
                    {"milestone": 7},
                ),
            ],
        )

    def test_malformed_pagination_item_fails_closed_and_post_is_not_retried(self):
        github = roadmap.GitHub("AlisinaDevelo/GHOSTRACE")
        malformed = SimpleNamespace(
            returncode=0,
            stderr="",
            stdout=json.dumps([{}, "bad"]),
        )
        with mock.patch.object(roadmap.subprocess, "run", return_value=malformed):
            with self.assertRaisesRegex(roadmap.RoadmapError, "malformed item"):
                github.paginated("repos/AlisinaDevelo/GHOSTRACE/issues")

        limited = SimpleNamespace(returncode=1, stderr="HTTP 429", stdout="")
        with mock.patch.object(
            roadmap.subprocess, "run", return_value=limited
        ) as run, mock.patch.object(roadmap.time, "sleep") as sleep:
            with self.assertRaises(roadmap.RoadmapError):
                github.request(
                    "POST",
                    "repos/AlisinaDevelo/GHOSTRACE/labels",
                    {"name": "test"},
                )
        self.assertEqual(run.call_count, 1)
        sleep.assert_not_called()

    def test_synced_marker_is_recognized_and_unmanaged_issue_is_ignored(self):
        program = tiny_program()
        task = tiny_task()
        FakeGitHub.responses = {
            "labels": program["labels"],
            "milestones?state=all": [{"number": 7, "title": "M0"}],
            "issues?state=all": [
                fake_issue(
                    1,
                    (
                        "<!-- forge-task:v1 id=0001 "
                        f"sync={roadmap.forge_task_sync_hash(task)} -->"
                    ),
                    milestone={"number": 7, "title": "M0"},
                ),
                fake_issue(2, "ordinary community issue"),
            ],
        }
        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={"0001": 1}
        ), mock.patch.object(roadmap, "GitHub", FakeGitHub):
            operations = roadmap.metadata_plan(
                program, [task], program["repository"], "0" * 64
            )
        self.assertFalse(
            any(
                operation["action"] in {"invalid_marker", "marker_missing"}
                for operation in operations
            )
        )

    def test_missing_milestone_is_a_planned_two_phase_apply_not_a_blocker(self):
        program = tiny_program()
        task = tiny_task()
        FakeGitHub.responses = {
            "labels": program["labels"],
            "milestones?state=all": [],
            "issues?state=all": [
                fake_issue(
                    1,
                    (
                        "<!-- forge-task:v1 id=0001 "
                        f"sync={roadmap.forge_task_sync_hash(task)} -->"
                    ),
                )
            ],
        }
        with mock.patch.object(roadmap, "validate"), mock.patch.object(
            roadmap, "load_sync_mapping", return_value={"0001": 1}
        ), mock.patch.object(roadmap, "GitHub", FakeGitHub):
            operations = roadmap.metadata_plan(
                program, [task], program["repository"], "0" * 64
            )
        self.assertTrue(any(item["action"] == "create_milestone" for item in operations))
        self.assertTrue(any(item["action"] == "defer_issue_metadata" for item in operations))
        self.assertEqual(roadmap.plan_blockers(operations), [])

    def test_malformed_program_metadata_is_rejected(self):
        program = tiny_program()
        program["labels"][0]["color"] = "not-a-color"
        with self.assertRaises(roadmap.RoadmapError):
            roadmap.validate(program, [])

    def test_milestone_dates_must_be_strictly_ordered(self):
        program = roadmap.load_program()
        program["milestones"][1]["due_on"] = program["milestones"][0]["due_on"]
        with self.assertRaises(roadmap.RoadmapError):
            roadmap.validate(program, roadmap.load_tasks())

    def test_dependency_cannot_target_a_later_milestone(self):
        program = roadmap.load_program()
        tasks = roadmap.load_tasks()
        by_id = {task.task_id: task for task in tasks}
        by_id["0006"] = replace(by_id["0006"], release="M1")
        with self.assertRaisesRegex(roadmap.RoadmapError, "later milestone"):
            roadmap.validate(program, list(by_id.values()))

    def test_every_task_must_feed_the_declared_final_gate(self):
        program = roadmap.load_program()
        tasks = roadmap.load_tasks()
        by_id = {task.task_id: task for task in tasks}
        by_id["0038"] = replace(
            by_id["0038"],
            depends_on=tuple(item for item in by_id["0038"].depends_on if item != "0047"),
        )
        with self.assertRaisesRegex(roadmap.RoadmapError, "not reachable from program gate"):
            roadmap.validate(program, list(by_id.values()))


if __name__ == "__main__":
    unittest.main()
