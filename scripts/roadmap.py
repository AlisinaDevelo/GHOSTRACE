#!/usr/bin/env python3
"""Validate the roadmap and synchronize narrowly scoped GitHub metadata.

The versioned task files own issue identity, content, state, and relationships.
GitHub exposes task titles and acceptance evidence publicly; internal Forge
identity and routing metadata is never part of a public issue body. Labels,
milestones, assignments, and metadata hygiene are reconciled idempotently here.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import stat
import subprocess
import sys
import time
import urllib.parse
from datetime import date, datetime, timezone
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
PROGRAM_PATH = ROOT / "planning" / "program.json"
TASKS_DIR = ROOT / ".forge" / "tasks"
SYNC_STATE_PATH = ROOT / ".forge" / "github-sync.json"
ROADMAP_PATH = ROOT / "docs" / "ROADMAP.md"
STATUSES = {"backlog", "ready", "in-progress", "review", "done", "blocked"}
MODELS = {"human"}
PROGRAM_SCHEMA_VERSION = 1
SYNC_SCHEMA_VERSION = 1
TASK_ID_RE = re.compile(r"[0-9]{4}\Z")
TASK_FILE_RE = re.compile(r"[0-9]{4}-[a-z0-9][a-z0-9-]*\.md\Z")
REPOSITORY_RE = re.compile(
    r"[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?/[A-Za-z0-9](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?\Z"
)
DATE_RE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}\Z")
DATETIME_RE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")
COLOR_RE = re.compile(r"[0-9a-fA-F]{6}\Z")
MARKER_RE = re.compile(
    r"<!-- forge-task:v1 id=([0-9]{4})(?: sync=([0-9a-f]{64}))? -->"
)
MARKER_HINT_RE = re.compile(r"forge-task:v1", re.IGNORECASE)
PUBLIC_ROUTING_LINE_RE = re.compile(
    r"^\s*(?:Assigned|Depends on|Parent):\s*.*$", re.IGNORECASE
)
FINAL_GATE_TASK_IDS = ("0153",)
SYNC_SCOPE = ("labels", "milestones", "assignees", "public-issue-bodies")
OWNER_MANAGED_STATUSES = frozenset({"ready", "in-progress", "review"})
MUTATION_INTERVAL_SECONDS = 1.0
MANAGED_EXACT = set()
KNOWN_TASK_FIELDS = {
    "id",
    "title",
    "status",
    "agent",
    "model",
    "release",
    "parent",
    "depends_on",
    "change",
    "workstream",
    "type",
    "priority",
    "risks",
    "platform",
}
PROGRAM_KEYS = {
    "schema_version",
    "project",
    "repository",
    "owner",
    "horizon",
    "milestones",
    "labels",
}


class RoadmapError(RuntimeError):
    pass


def _strict_regular(path: Path, kind: str) -> None:
    """Reject links and special files before reading or overwriting metadata."""
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError as exc:
        raise RoadmapError(f"{kind} does not exist: {path}") from exc
    if stat.S_ISLNK(mode):
        raise RoadmapError(f"{kind} must not be a symlink: {path}")
    if not stat.S_ISREG(mode):
        raise RoadmapError(f"{kind} must be a regular file: {path}")


def _strict_directory(path: Path, kind: str) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError as exc:
        raise RoadmapError(f"{kind} does not exist: {path}") from exc
    if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
        raise RoadmapError(f"{kind} must be a non-symlink directory: {path}")


def _require_string(value: Any, name: str, *, nonempty: bool = True) -> str:
    if type(value) is not str or (nonempty and not value.strip()):
        raise RoadmapError(f"{name} must be a non-empty string")
    return value


def _parse_date(value: Any, name: str) -> date:
    text = _require_string(value, name)
    if not DATE_RE.fullmatch(text):
        raise RoadmapError(f"{name} must be YYYY-MM-DD")
    try:
        return date.fromisoformat(text)
    except ValueError as exc:
        raise RoadmapError(f"{name} is not a valid calendar date: {text}") from exc


def _parse_due_on(value: Any, name: str) -> datetime:
    text = _require_string(value, name)
    if not DATETIME_RE.fullmatch(text):
        raise RoadmapError(f"{name} must be an RFC3339 UTC timestamp")
    try:
        parsed = datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as exc:
        raise RoadmapError(f"{name} is not a valid timestamp: {text}") from exc
    return parsed.replace(tzinfo=timezone.utc)


def _validate_repository(repository: Any, name: str = "repository") -> str:
    value = _require_string(repository, name)
    if not REPOSITORY_RE.fullmatch(value):
        raise RoadmapError(f"{name} must be a strict OWNER/REPO value")
    return value


def _read_text(path: Path, kind: str) -> str:
    _strict_regular(path, kind)
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise RoadmapError(f"cannot read {kind} {path}: {exc}") from exc


def _json_no_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise RoadmapError(f"duplicate JSON metadata key: {key}")
        result[key] = value
    return result


@dataclass(frozen=True)
class Task:
    task_id: str
    title: str
    status: str
    agent: str
    model: str
    release: str
    parent: str | None
    depends_on: tuple[str, ...]
    workstream: str
    kind: str
    priority: str
    risks: tuple[str, ...]
    platform: str
    goal: str
    criteria: tuple[tuple[bool, str], ...]
    context: str
    notes: str
    path: Path
    source_sha256: str


def parse_list(value: str) -> tuple[str, ...]:
    value = value.strip()
    if not value.startswith("[") or not value.endswith("]"):
        raise RoadmapError(f"expected bracketed list, got {value!r}")
    inner = value[1:-1].strip()
    if not inner:
        return ()
    items = tuple(item.strip() for item in inner.split(","))
    if any(not item or "," in item or "[" in item or "]" in item for item in items):
        raise RoadmapError(f"malformed list item in {value!r}")
    return items


def section(body: str, heading: str) -> str:
    match = re.search(
        rf"^##\s+{re.escape(heading)}\s*$([\s\S]*?)(?=^##\s+|\Z)",
        body,
        re.MULTILINE | re.IGNORECASE,
    )
    return match.group(1).strip() if match else ""


def parse_task(path: Path) -> Task:
    _strict_regular(path, "task file")
    text = _read_text(path, "task file")
    match = re.match(r"\A---\n([\s\S]*?)\n---\n([\s\S]*)\Z", text)
    if not match:
        raise RoadmapError(f"{path}: malformed frontmatter")
    fields: dict[str, str] = {}
    for line in match.group(1).splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            raise RoadmapError(f"{path}: malformed frontmatter line {line!r}")
        key, value = line.split(":", 1)
        key = key.strip()
        if key not in KNOWN_TASK_FIELDS:
            raise RoadmapError(f"{path}: unknown frontmatter field {key!r}")
        if key in fields:
            raise RoadmapError(f"{path}: duplicate frontmatter field {key!r}")
        fields[key] = value.strip().strip('"')
    required = {
        "id",
        "title",
        "status",
        "agent",
        "model",
        "release",
        "depends_on",
        "workstream",
        "type",
        "priority",
        "risks",
        "platform",
        "change",
    }
    missing = sorted(required - fields.keys())
    if missing:
        raise RoadmapError(f"{path}: missing frontmatter fields: {', '.join(missing)}")
    if not TASK_ID_RE.fullmatch(fields["id"]):
        raise RoadmapError(f"{path}: id must be exactly four digits")
    if not fields["title"] or "\n" in fields["title"]:
        raise RoadmapError(f"{path}: title must be non-empty")
    if "parent" in fields:
        parent = fields["parent"]
        if parent != "null" and not TASK_ID_RE.fullmatch(parent):
            raise RoadmapError(f"{path}: parent must be null or a four-digit task id")
    if fields["change"] != "null" and not re.fullmatch(r"[A-Za-z0-9_.:/-]+", fields["change"]):
        raise RoadmapError(f"{path}: change must be null or a stable reference")
    for heading in ("Goal", "Acceptance criteria", "Context", "Notes"):
        count = len(
            re.findall(rf"^##\s+{re.escape(heading)}\s*$", match.group(2), re.MULTILINE | re.IGNORECASE)
        )
        if count != 1:
            raise RoadmapError(f"{path}: expected exactly one {heading} section")
    criteria: list[tuple[bool, str]] = []
    criteria_body = section(match.group(2), "Acceptance criteria")
    for line in criteria_body.splitlines():
        if not line.strip():
            continue
        item = re.match(r"^[-*]\s+\[([ xX])\]\s+(.+)$", line)
        if not item:
            raise RoadmapError(f"{path}: malformed acceptance criterion {line!r}")
        criteria.append((item.group(1).lower() == "x", item.group(2).strip()))
    return Task(
        task_id=fields["id"],
        title=fields["title"],
        status=fields["status"],
        agent=fields["agent"],
        model=fields["model"],
        release=fields["release"],
        parent=(fields.get("parent") or None) if fields.get("parent") != "null" else None,
        depends_on=parse_list(fields["depends_on"]),
        workstream=fields["workstream"],
        kind=fields["type"],
        priority=fields["priority"],
        risks=parse_list(fields["risks"]),
        platform=fields["platform"],
        goal=section(match.group(2), "Goal"),
        criteria=tuple(criteria),
        context=section(match.group(2), "Context"),
        notes=section(match.group(2), "Notes"),
        path=path,
        source_sha256=hashlib.sha256(text.encode("utf-8")).hexdigest(),
    )


def validate_program_shape(program: Any) -> dict[str, Any]:
    if type(program) is not dict:
        raise RoadmapError("program metadata must be a JSON object")
    unknown = sorted(set(program) - PROGRAM_KEYS)
    missing = sorted(PROGRAM_KEYS - set(program))
    if unknown:
        raise RoadmapError(f"program metadata has unknown fields: {', '.join(unknown)}")
    if missing:
        raise RoadmapError(f"program metadata is missing fields: {', '.join(missing)}")
    if type(program["schema_version"]) is not int or program["schema_version"] != PROGRAM_SCHEMA_VERSION:
        raise RoadmapError("unsupported program schema version")
    if _require_string(program["project"], "program.project") != "GHOSTRACE":
        raise RoadmapError("program.project must be GHOSTRACE")
    repository = _validate_repository(program["repository"], "program.repository")
    owner = _require_string(program["owner"], "program.owner")
    configured_owner, _ = repository.split("/", 1)
    if owner != configured_owner:
        raise RoadmapError("program.owner must exactly match the repository owner")

    horizon = program["horizon"]
    if type(horizon) is not dict:
        raise RoadmapError("program.horizon must be an object")
    if set(horizon) != {"starts_on", "ends_on", "minimum_task_count"}:
        raise RoadmapError("program.horizon has an invalid schema")
    starts_on = _parse_date(horizon["starts_on"], "program.horizon.starts_on")
    ends_on = _parse_date(horizon["ends_on"], "program.horizon.ends_on")
    if starts_on >= ends_on:
        raise RoadmapError("program horizon must have starts_on before ends_on")
    if type(horizon["minimum_task_count"]) is not int or horizon["minimum_task_count"] < 0:
        raise RoadmapError("program.horizon.minimum_task_count must be a non-negative integer")

    milestones = program["milestones"]
    if type(milestones) is not list or not milestones:
        raise RoadmapError("program.milestones must be a non-empty list")
    milestone_dates: list[datetime] = []
    milestone_titles: list[str] = []
    for index, milestone in enumerate(milestones):
        if type(milestone) is not dict or set(milestone) != {"title", "due_on", "description"}:
            raise RoadmapError(f"program.milestones[{index}] has an invalid schema")
        title = _require_string(milestone["title"], f"program.milestones[{index}].title")
        if not re.fullmatch(r"M[0-9]+", title):
            raise RoadmapError(f"program.milestones[{index}].title must be M<number>")
        due_on = _parse_due_on(milestone["due_on"], f"program.milestones[{index}].due_on")
        description = _require_string(milestone["description"], f"program.milestones[{index}].description")
        if due_on.date() < starts_on or due_on.date() > ends_on:
            raise RoadmapError(f"program.milestones[{index}].due_on is outside the program horizon")
        milestone_titles.append(title)
        milestone_dates.append(due_on)
        _ = description
    if len(set(milestone_titles)) != len(milestone_titles):
        raise RoadmapError("milestone titles are not unique")
    if any(current.date() <= previous.date() for previous, current in zip(milestone_dates, milestone_dates[1:])):
        raise RoadmapError("milestone due dates are not strictly ordered")
    if milestone_dates[-1].date() != ends_on:
        raise RoadmapError("last milestone does not end on the program horizon")

    labels = program["labels"]
    if type(labels) is not list or not labels:
        raise RoadmapError("program.labels must be a non-empty list")
    label_names: list[str] = []
    for index, label in enumerate(labels):
        if type(label) is not dict or set(label) != {"name", "color", "description"}:
            raise RoadmapError(f"program.labels[{index}] has an invalid schema")
        name = _require_string(label["name"], f"program.labels[{index}].name")
        color = _require_string(label["color"], f"program.labels[{index}].color")
        _require_string(label["description"], f"program.labels[{index}].description")
        if not COLOR_RE.fullmatch(color):
            raise RoadmapError(f"program.labels[{index}].color must be six hexadecimal digits")
        if any(character.isspace() for character in name):
            raise RoadmapError(f"program.labels[{index}].name must not contain whitespace")
        label_names.append(name)
    if len(set(label_names)) != len(label_names):
        raise RoadmapError("label names are not unique")
    return program


def load_program() -> dict[str, Any]:
    try:
        program = json.loads(_read_text(PROGRAM_PATH, "program metadata"), object_pairs_hook=_json_no_duplicate_keys)
    except (OSError, json.JSONDecodeError) as exc:
        raise RoadmapError(f"cannot load {PROGRAM_PATH}: {exc}") from exc
    return validate_program_shape(program)


def load_tasks() -> list[Task]:
    _strict_directory(TASKS_DIR, "task directory")
    index_path = TASKS_DIR / "README.md"
    _strict_regular(index_path, "task index")
    paths: list[Path] = []
    for path in sorted(TASKS_DIR.iterdir(), key=lambda item: item.name):
        if path.name == index_path.name:
            continue
        try:
            mode = path.lstat().st_mode
        except OSError as exc:
            raise RoadmapError(f"cannot inspect task path {path}: {exc}") from exc
        if stat.S_ISLNK(mode):
            raise RoadmapError(f"task path must not be a symlink: {path}")
        if not stat.S_ISREG(mode):
            raise RoadmapError(f"task path must be a regular file: {path}")
        if not TASK_FILE_RE.fullmatch(path.name):
            raise RoadmapError(f"unexpected task path: {path.name}")
        paths.append(path)
    if not paths:
        raise RoadmapError(f"task directory contains no task files: {TASKS_DIR}")
    return [parse_task(path) for path in paths]


def forge_task_sync_hash(task: Task) -> str:
    """Match Forge task-ledger's canonical per-task content hash.

    Forge hashes criterion text, not checkbox glyphs. Validation therefore requires
    every non-done task to be fully unchecked and every done task to be fully checked,
    making checkbox state a deterministic consequence of the hashed status.
    """
    document = {
        "id": task.task_id,
        "title": task.title.strip(),
        "status": task.status,
        "agent": task.agent.strip(),
        "model": task.model.strip(),
        "depends_on": sorted(task.depends_on),
        "parent": task.parent,
        "release": task.release,
        "goal": task.goal.strip(),
        "acceptance": [text.strip() for _, text in task.criteria],
        "context": task.context.strip(),
        "notes": task.notes.strip(),
    }
    canonical = json.dumps(
        document,
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def compute_task_tree_digest(tasks: list[Task]) -> str:
    """Bind a parity receipt to the exact bytes parsed into this task snapshot."""
    entries = []
    for task in sorted(tasks, key=lambda item: item.task_id):
        if not re.fullmatch(r"[0-9a-f]{64}", task.source_sha256):
            raise RoadmapError(
                f"{task.task_id}: source snapshot must contain a SHA-256 digest"
            )
        entries.append(
            {
                "id": task.task_id,
                "path": task.path.name,
                "sha256": task.source_sha256,
            }
        )
    document = {"schema_version": PROGRAM_SCHEMA_VERSION, "tasks": entries}
    canonical = json.dumps(
        document,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def assert_acyclic(tasks: dict[str, Task], edge: str) -> None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(task_id: str, trail: tuple[str, ...]) -> None:
        if task_id in visiting:
            cycle = " -> ".join((*trail, task_id))
            raise RoadmapError(f"{edge} cycle: {cycle}")
        if task_id in visited:
            return
        visiting.add(task_id)
        task = tasks[task_id]
        targets: Iterable[str]
        if edge == "dependency":
            targets = task.depends_on
        else:
            targets = (task.parent,) if task.parent else ()
        for target in targets:
            visit(target, (*trail, task_id))
        visiting.remove(task_id)
        visited.add(task_id)

    for task_id in tasks:
        visit(task_id, ())


def validate(program: dict[str, Any], tasks: list[Task]) -> dict[str, Any]:
    validate_program_shape(program)
    if type(tasks) is not list or any(not isinstance(task, Task) for task in tasks):
        raise RoadmapError("task metadata must be a list of Task values")
    errors: list[str] = []
    milestones = program["milestones"]
    labels = program["labels"]
    milestone_titles = [item["title"] for item in milestones]
    label_names = [item["name"] for item in labels]
    minimum = program["horizon"]["minimum_task_count"]
    if len(tasks) < minimum:
        errors.append(f"task count {len(tasks)} is below required minimum {minimum}")

    by_id = {task.task_id: task for task in tasks}
    if len(by_id) != len(tasks):
        errors.append("task identifiers are not unique")
    actual_ids = [task.task_id for task in tasks]
    if actual_ids != sorted(actual_ids):
        errors.append("task identifiers are not ordered")
    titles = [task.title for task in tasks]
    if len(set(titles)) != len(titles):
        errors.append("task titles are not unique")

    release_counts = {title: 0 for title in milestone_titles}
    release_index = {title: index for index, title in enumerate(milestone_titles)}
    status_counts = {status: 0 for status in sorted(STATUSES)}
    workstream_counts: dict[str, int] = {}
    for task in tasks:
        if not TASK_ID_RE.fullmatch(task.task_id):
            errors.append(f"{task.task_id}: task id must be exactly four digits")
        for field in (
            "title",
            "agent",
            "model",
            "release",
            "workstream",
            "kind",
            "priority",
            "platform",
            "goal",
            "context",
            "notes",
        ):
            value = getattr(task, field)
            if type(value) is not str or not value.strip():
                errors.append(f"{task.task_id}: {field} must be a non-empty string")
        if type(task.parent) is not str and task.parent is not None:
            errors.append(f"{task.task_id}: parent must be null or a string")
        if type(task.depends_on) is not tuple or any(type(item) is not str for item in task.depends_on):
            errors.append(f"{task.task_id}: depends_on must be a tuple of strings")
        if type(task.risks) is not tuple or any(type(item) is not str or not item.strip() for item in task.risks):
            errors.append(f"{task.task_id}: risks must be a tuple of non-empty strings")
        if (
            type(task.risks) is tuple
            and all(type(item) is str for item in task.risks)
            and len(task.risks) != len(set(task.risks))
        ):
            errors.append(f"{task.task_id}: duplicate risks")
        if not isinstance(task.path, Path):
            errors.append(f"{task.task_id}: path must be a filesystem path")
            continue
        if not re.fullmatch(r"[0-9a-f]{64}", task.source_sha256):
            errors.append(f"{task.task_id}: source snapshot digest is invalid")
        prefix = task.path.name.split("-", 1)[0]
        if prefix != task.task_id:
            errors.append(f"{task.path.name}: filename and task id differ")
        if task.status not in STATUSES:
            errors.append(f"{task.task_id}: unsupported status {task.status}")
        else:
            status_counts[task.status] += 1
        if task.model not in MODELS:
            errors.append(f"{task.task_id}: unsupported model tier {task.model}")
        if not task.agent:
            errors.append(f"{task.task_id}: missing assigned agent")
        if task.release not in release_counts:
            errors.append(f"{task.task_id}: unknown release {task.release}")
        else:
            release_counts[task.release] += 1
        if not task.title.strip() or "\n" in task.title:
            errors.append(f"{task.task_id}: title must be non-empty and single-line")
        if type(task.depends_on) is tuple and any(
            not TASK_ID_RE.fullmatch(dependency) for dependency in task.depends_on
        ):
            errors.append(f"{task.task_id}: dependency ids must be exactly four digits")
        if task.parent and not TASK_ID_RE.fullmatch(task.parent):
            errors.append(f"{task.task_id}: parent id must be exactly four digits")
        desired_names = {
            f"epic:{task.workstream}",
            f"type:{task.kind}",
            f"priority:{task.priority}",
            f"status:{task.status}",
        }
        desired_names.update(f"risk:{risk}" for risk in task.risks)
        if task.platform == "macos":
            desired_names.add("platform:macos")
        elif task.platform != "any":
            errors.append(f"{task.task_id}: unsupported platform {task.platform}")
        unknown_labels = sorted(desired_names - set(label_names))
        if unknown_labels:
            errors.append(f"{task.task_id}: labels missing from program: {', '.join(unknown_labels)}")
        if not task.goal or not task.context:
            errors.append(f"{task.task_id}: goal and context are required")
        if len(task.criteria) < 2:
            errors.append(f"{task.task_id}: at least two acceptance criteria are required")
        criterion_text = [item[1] for item in task.criteria]
        if len(criterion_text) != len(set(criterion_text)):
            errors.append(f"{task.task_id}: duplicate acceptance criteria")
        if (
            type(task.depends_on) is tuple
            and all(type(item) is str for item in task.depends_on)
            and len(task.depends_on) != len(set(task.depends_on))
        ):
            errors.append(f"{task.task_id}: duplicate dependencies")
        if len(task.depends_on) > 50:
            errors.append(f"{task.task_id}: exceeds GitHub's bounded dependency plan")
        for dependency in task.depends_on:
            if dependency not in by_id:
                errors.append(f"{task.task_id}: unknown dependency {dependency}")
            if dependency == task.task_id:
                errors.append(f"{task.task_id}: self dependency")
            elif dependency in by_id and task.release in release_index and by_id[dependency].release in release_index:
                if release_index[by_id[dependency].release] > release_index[task.release]:
                    errors.append(
                        f"{task.task_id}: dependency {dependency} targets a later milestone "
                        f"({by_id[dependency].release} after {task.release})"
                    )
        if task.parent:
            if task.parent not in by_id:
                errors.append(f"{task.task_id}: unknown parent {task.parent}")
            if task.parent == task.task_id:
                errors.append(f"{task.task_id}: self parent")
        if task.status == "done" and not all(checked for checked, _ in task.criteria):
            errors.append(f"{task.task_id}: done task has unchecked acceptance criteria")
        if task.status != "done" and any(checked for checked, _ in task.criteria):
            errors.append(
                f"{task.task_id}: non-done task has checked acceptance criteria"
            )
        if task.status == "ready":
            incomplete = [dep for dep in task.depends_on if dep in by_id and by_id[dep].status != "done"]
            if incomplete:
                errors.append(f"{task.task_id}: ready task has incomplete dependencies: {', '.join(incomplete)}")
        workstream_counts[task.workstream] = workstream_counts.get(task.workstream, 0) + 1

    if not errors:
        try:
            assert_acyclic(by_id, "dependency")
            assert_acyclic(by_id, "parent")
        except RoadmapError as exc:
            errors.append(str(exc))
    if any(count == 0 for count in release_counts.values()):
        errors.append("every milestone must contain at least one task")
    gate_ids = tuple(FINAL_GATE_TASK_IDS)
    missing_gates = [task_id for task_id in gate_ids if task_id not in by_id]
    if missing_gates:
        errors.append(f"declared program gates are missing: {', '.join(missing_gates)}")
    else:
        reachable: set[str] = set()
        pending = list(gate_ids)
        while pending:
            task_id = pending.pop()
            if task_id in reachable:
                continue
            reachable.add(task_id)
            pending.extend(dependency for dependency in by_id[task_id].depends_on if dependency in by_id)
        orphans = sorted(set(by_id) - reachable)
        if orphans:
            errors.append(
                "tasks not reachable from program gate "
                f"{', '.join(gate_ids)}: {', '.join(orphans)}"
            )
    roadmap = _read_text(ROADMAP_PATH, "ROADMAP.md") if ROADMAP_PATH.exists() else ""
    if program["horizon"]["ends_on"] not in roadmap:
        errors.append("ROADMAP.md does not state the program end date")
    if str(minimum) not in roadmap:
        errors.append("ROADMAP.md does not state the minimum task count")
    if errors:
        raise RoadmapError("\n".join(errors))
    return {
        "tasks": len(tasks),
        "milestones": len(milestones),
        "starts_on": program["horizon"]["starts_on"],
        "ends_on": program["horizon"]["ends_on"],
        "release_counts": release_counts,
        "status_counts": status_counts,
        "workstream_counts": dict(sorted(workstream_counts.items())),
        "dependency_edges": sum(len(task.depends_on) for task in tasks),
        "parent_edges": sum(task.parent is not None for task in tasks),
        "program_gates": list(gate_ids),
    }


def render_index(tasks: list[Task]) -> str:
    lines = [
        "# GHOSTRACE task ledger",
        "",
        "This is the versioned source of truth for the 2026–2031 program. A task "
        "is done only when its acceptance criteria have verified evidence; GitHub "
        "mirrors this ledger with milestones, labels, native sub-issues, and "
        "blocked-by relationships.",
        "",
        "| id | title | release | status | workstream | owner | parent | depends_on |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for task in tasks:
        title = task.title.replace("|", "\\|")
        parent = task.parent or "—"
        dependencies = ", ".join(task.depends_on) or "—"
        lines.append(
            f"| {task.task_id} | {title} | {task.release} | {task.status} | {task.workstream} | "
            f"{task.agent} | {parent} | {dependencies} |"
        )
    return "\n".join(lines) + "\n"


class GitHub:
    def __init__(self, repository: str) -> None:
        self.repository = _validate_repository(repository)

    def request(self, method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
        command = [
            "gh",
            "api",
            path,
            "--method",
            method,
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
        ]
        input_data = None
        if payload is not None:
            command.extend(["--input", "-"])
            input_data = json.dumps(payload)
        for attempt in range(3):
            result = subprocess.run(
                command,
                input=input_data,
                capture_output=True,
                text=True,
                check=False,
                timeout=30,
            )
            if result.returncode == 0:
                return json.loads(result.stdout) if result.stdout.strip() else None
            message = (
                "\n".join(
                    part.strip()
                    for part in (result.stderr, result.stdout)
                    if part.strip()
                )
                or "GitHub request failed"
            )
            status_codes = {
                int(code)
                for code in re.findall(
                    r"\b(?:HTTP(?:/\S+)?\s+|status(?:\s+code)?[=: ]+)([0-9]{3})\b",
                    message,
                    re.IGNORECASE,
                )
            }
            retryable = bool(status_codes & {408, 425, 429, 500, 502, 503, 504}) or any(
                term in message.lower() for term in ("rate limit", "abuse", "temporarily", "secondary rate")
            )
            safe_to_retry = method in {"GET", "HEAD", "PUT", "PATCH", "DELETE"}
            if attempt < 2 and retryable and safe_to_retry:
                retry_after = re.search(r"retry-after\s*:\s*([0-9]+(?:\.[0-9]+)?)", message, re.IGNORECASE)
                delay = float(retry_after.group(1)) if retry_after else float(2**attempt)
                time.sleep(min(max(delay, 0.0), 30.0))
                continue
            raise RoadmapError(f"GitHub {method} {path}: {message}")
        raise RoadmapError("unreachable GitHub retry state")

    def paginated(self, path: str) -> list[dict[str, Any]]:
        values: list[dict[str, Any]] = []
        separator = "&" if "?" in path else "?"
        for page in range(1, 101):
            data = self.request("GET", f"{path}{separator}per_page=100&page={page}")
            if not isinstance(data, list):
                raise RoadmapError(f"GitHub pagination returned non-list for {path}")
            if any(type(item) is not dict for item in data):
                raise RoadmapError(
                    f"GitHub pagination returned a malformed item for {path}"
                )
            values.extend(data)
            if len(data) < 100:
                return values
        raise RoadmapError(f"GitHub pagination exceeded bound for {path}")


def load_sync_mapping(
    expected_repository: str | None = None,
    task_ids: Iterable[str] | None = None,
) -> dict[str, int]:
    expected = (
        _validate_repository(expected_repository, "expected repository")
        if expected_repository is not None
        else None
    )
    try:
        sync_mode = SYNC_STATE_PATH.lstat().st_mode
    except FileNotFoundError:
        return {}
    if stat.S_ISLNK(sync_mode) or not stat.S_ISREG(sync_mode):
        raise RoadmapError(f"GitHub sync mapping must be a regular non-symlink file: {SYNC_STATE_PATH}")
    data = json.loads(_read_text(SYNC_STATE_PATH, "GitHub sync mapping"), object_pairs_hook=_json_no_duplicate_keys)
    if type(data) is not dict:
        raise RoadmapError("GitHub sync mapping must be a JSON object")
    if set(data) - {"schema_version", "repository", "tasks", "completed_operations"}:
        raise RoadmapError("GitHub sync mapping has unknown fields")
    if set(data) < {"schema_version", "repository", "tasks"}:
        raise RoadmapError("GitHub sync mapping is missing required fields")
    if type(data["schema_version"]) is not int or data["schema_version"] != SYNC_SCHEMA_VERSION:
        raise RoadmapError("unsupported GitHub sync mapping schema version")
    repository = _validate_repository(data["repository"], "GitHub sync mapping.repository")
    if expected is not None:
        if repository != expected:
            raise RoadmapError(
                f"GitHub sync mapping repository {repository!r} does not match configured repository {expected!r}"
            )
    completed_operations = data.get("completed_operations", [])
    if type(completed_operations) is not list or any(
        type(item) is not str or not re.fullmatch(r"[0-9a-f]{64}", item) for item in completed_operations
    ):
        raise RoadmapError("GitHub sync mapping.completed_operations must contain SHA-256 digests")
    raw_tasks = data["tasks"]
    if type(raw_tasks) is not dict:
        raise RoadmapError("GitHub sync mapping.tasks must be an object")
    known_task_ids = set(task_ids) if task_ids is not None else None
    result: dict[str, int] = {}
    seen_issues: dict[int, str] = {}
    for task_id, value in sorted(raw_tasks.items()):
        if not isinstance(task_id, str) or not TASK_ID_RE.fullmatch(task_id):
            raise RoadmapError(f"GitHub sync mapping has invalid task id {task_id!r}")
        if known_task_ids is not None and task_id not in known_task_ids:
            raise RoadmapError(f"GitHub sync mapping contains stale task id {task_id}")
        if type(value) is not dict or set(value) != {"issue"}:
            raise RoadmapError(f"GitHub sync mapping entry {task_id} has an invalid schema")
        issue = value["issue"]
        if type(issue) is not int or issue <= 0:
            raise RoadmapError(f"GitHub sync mapping entry {task_id} has an invalid issue number")
        previous = seen_issues.get(issue)
        if previous is not None:
            raise RoadmapError(f"GitHub sync mapping maps issue {issue} to both {previous} and {task_id}")
        seen_issues[issue] = task_id
        result[task_id] = issue
    return result


def load_forge_parity_plan(
    path: Path,
    expected_repository: str,
    expected_task_tree_digest: str,
) -> str:
    """Verify an inspected, clean Forge-native plan and return its canonical digest."""
    expected = _validate_repository(expected_repository, "expected repository")
    if not re.fullmatch(r"[0-9a-f]{64}", expected_task_tree_digest):
        raise RoadmapError("expected task-tree digest must be a SHA-256 digest")
    try:
        document = json.loads(
            _read_text(path, "Forge parity plan"),
            object_pairs_hook=_json_no_duplicate_keys,
        )
    except json.JSONDecodeError as exc:
        raise RoadmapError(f"cannot load Forge parity plan {path}: {exc}") from exc
    if type(document) is not dict or set(document) != {
        "authority",
        "operations",
        "repository",
        "task_tree_digest",
    }:
        raise RoadmapError(
            "Forge parity plan must contain only authority, operations, repository, "
            "and task_tree_digest"
        )
    repository = _validate_repository(
        document["repository"], "Forge parity plan.repository"
    )
    if repository != expected:
        raise RoadmapError(
            f"Forge parity plan repository {repository!r} does not match {expected!r}"
        )
    task_tree_digest = _require_string(
        document["task_tree_digest"], "Forge parity plan.task_tree_digest"
    )
    if not re.fullmatch(r"[0-9a-f]{64}", task_tree_digest):
        raise RoadmapError("Forge parity plan.task_tree_digest must be a SHA-256 digest")
    if task_tree_digest != expected_task_tree_digest:
        raise RoadmapError(
            "Forge parity plan does not match the current versioned task tree"
        )
    if document["authority"] != "local":
        raise RoadmapError("Forge parity plan must use local authority")
    operations = document["operations"]
    if type(operations) is not list or any(type(operation) is not dict for operation in operations):
        raise RoadmapError("Forge parity plan operations must be a list of objects")
    if operations:
        raise RoadmapError(
            f"Forge-native parity has {len(operations)} pending operation(s); synchronize it first"
        )
    canonical = json.dumps(
        document,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def verify_local_snapshot_current(
    program: dict[str, Any], tasks: list[Task]
) -> None:
    """Fail if tracked planning inputs changed after this process parsed them."""
    current_program = load_program()
    if current_program != program:
        raise RoadmapError(
            "program metadata changed after it was loaded; inspect a new plan"
        )
    current_tasks = load_tasks()
    if compute_task_tree_digest(current_tasks) != compute_task_tree_digest(tasks):
        raise RoadmapError(
            "task tree changed after it was loaded; inspect a new plan"
        )


def verify_live_forge_parity(
    forge_tasks_path: Path,
    repository: str,
) -> str:
    """Re-run Forge's read-only plan immediately before metadata mutation."""
    _strict_regular(forge_tasks_path, "Forge task-ledger script")
    configured_repository = _validate_repository(repository)
    command = [
        sys.executable,
        str(forge_tasks_path.resolve()),
        "--repo",
        configured_repository,
        "--tasks-dir",
        str(TASKS_DIR),
        "--json",
        "plan",
    ]
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        check=False,
        timeout=300,
    )
    if result.returncode:
        detail = (result.stderr.strip() or result.stdout.strip() or "plan failed")[
            :2000
        ]
        raise RoadmapError(f"live Forge parity check failed: {detail}")
    try:
        document = json.loads(
            result.stdout,
            object_pairs_hook=_json_no_duplicate_keys,
        )
    except json.JSONDecodeError as exc:
        raise RoadmapError("live Forge parity check returned invalid JSON") from exc
    if type(document) is not dict or set(document) != {"authority", "operations"}:
        raise RoadmapError("live Forge parity check returned an invalid schema")
    if document["authority"] != "local":
        raise RoadmapError("live Forge parity check did not use local authority")
    operations = document["operations"]
    if type(operations) is not list or any(
        type(operation) is not dict for operation in operations
    ):
        raise RoadmapError("live Forge parity operations must be a list of objects")
    if operations:
        raise RoadmapError(
            "live Forge parity has "
            f"{len(operations)} pending operation(s); synchronize it first"
        )
    canonical = json.dumps(
        document,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def desired_labels(task: Task) -> list[str]:
    values = [
        f"epic:{task.workstream}",
        f"type:{task.kind}",
        f"priority:{task.priority}",
    ]
    values.extend(f"risk:{risk}" for risk in task.risks)
    if task.platform == "macos":
        values.append("platform:macos")
    values.append(f"status:{task.status}")
    return values


def is_managed_label(name: str, managed_labels: set[str] | None = None) -> bool:
    """Only labels explicitly declared by program.json are managed."""
    return name in (MANAGED_EXACT if managed_labels is None else managed_labels)


def _blocking(action: str, **values: Any) -> dict[str, Any]:
    return {"action": action, "blocking": True, **values}


def _named_remote_items(items: Any, kind: str, field: str) -> dict[str, dict[str, Any]]:
    if type(items) is not list:
        raise RoadmapError(f"GitHub {kind} response must be a list")
    result: dict[str, dict[str, Any]] = {}
    for item in items:
        if type(item) is not dict or type(item.get(field)) is not str or not item[field].strip():
            raise RoadmapError(f"GitHub {kind} response contains malformed {field}")
        name = item[field]
        if name in result:
            raise RoadmapError(f"GitHub {kind} response contains duplicate {field} {name!r}")
        result[name] = item
    return result


def _issue_items(items: Any) -> dict[int, dict[str, Any]]:
    if type(items) is not list:
        raise RoadmapError("GitHub issues response must be a list")
    result: dict[int, dict[str, Any]] = {}
    for item in items:
        if type(item) is not dict or "pull_request" in item:
            continue
        number = item.get("number")
        if type(number) is not int or number <= 0:
            raise RoadmapError("GitHub issues response contains an invalid issue number")
        if number in result:
            raise RoadmapError(f"GitHub issues response contains duplicate issue {number}")
        for field in ("labels", "assignees"):
            if type(item.get(field)) is not list:
                raise RoadmapError(f"GitHub issue {number} has malformed {field}")
        body = item.get("body")
        if body is not None and type(body) is not str:
            raise RoadmapError(f"GitHub issue {number} has malformed body")
        result[number] = item
    return result


def _issue_markers(body: str | None) -> list[tuple[str, str | None]]:
    if body is None:
        return []
    return [
        (match.group(1), match.group(2))
        for match in MARKER_RE.finditer(body)
    ]


def sanitize_public_issue_body(body: str | None) -> str:
    """Remove internal Forge identity and routing metadata from public bodies.

    Task titles remain the stable public identity used by the metadata publisher.
    This deliberately removes only metadata lines and leaves the goal,
    acceptance criteria, context, notes, and ordinary prose unchanged.
    """
    if body is None:
        return ""
    kept: list[str] = []
    for line in body.splitlines():
        if MARKER_HINT_RE.search(line) or PUBLIC_ROUTING_LINE_RE.fullmatch(line):
            continue
        kept.append(line.rstrip())
    content = "\n".join(kept).strip()
    return f"{content}\n" if content else ""


def _validate_issue_fields(issue_number: int, issue: dict[str, Any]) -> None:
    for field in ("title", "state", "updated_at"):
        if type(issue.get(field)) is not str or not issue[field].strip():
            raise RoadmapError(
                f"GitHub issue {issue_number} has malformed {field} metadata"
            )
    body = issue.get("body")
    if body is not None and type(body) is not str:
        raise RoadmapError(f"GitHub issue {issue_number} has malformed body metadata")
    for item in issue["labels"]:
        if type(item) is not dict or type(item.get("name")) is not str or not item["name"].strip():
            raise RoadmapError(f"GitHub issue {issue_number} has malformed label metadata")
    for item in issue["assignees"]:
        if type(item) is not dict or type(item.get("login")) is not str or not item["login"].strip():
            raise RoadmapError(f"GitHub issue {issue_number} has malformed assignee metadata")
    milestone = issue.get("milestone")
    if milestone is not None and (
        type(milestone) is not dict or type(milestone.get("number")) is not int or milestone["number"] <= 0
    ):
        raise RoadmapError(f"GitHub issue {issue_number} has malformed milestone metadata")


def _configured_repository(program: dict[str, Any], repository: str) -> str:
    validate_program_shape(program)
    configured = _validate_repository(program["repository"], "program.repository")
    requested = _validate_repository(repository)
    if requested != configured:
        raise RoadmapError(f"repository {requested!r} does not match configured repository {configured!r}")
    return requested


def metadata_plan(
    program: dict[str, Any],
    tasks: list[Task],
    repository: str,
    forge_plan_digest: str | None = None,
) -> list[dict[str, Any]]:
    _configured_repository(program, repository)
    validate(program, tasks)
    if forge_plan_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", forge_plan_digest):
        raise RoadmapError("Forge parity plan digest must be a SHA-256 digest")
    mapping = load_sync_mapping(repository, (task.task_id for task in tasks))
    github = GitHub(repository)
    prefix = f"repos/{repository}"
    current_labels = _named_remote_items(github.paginated(f"{prefix}/labels"), "labels", "name")
    current_milestones = _named_remote_items(
        github.paginated(f"{prefix}/milestones?state=all"), "milestones", "title"
    )
    current_issues = _issue_items(github.paginated(f"{prefix}/issues?state=all"))
    for name, label in current_labels.items():
        if type(label.get("color")) is not str:
            raise RoadmapError(f"GitHub label {name!r} has malformed color metadata")
        if label.get("description") is not None and type(label.get("description")) is not str:
            raise RoadmapError(f"GitHub label {name!r} has malformed description metadata")
    for title, milestone in current_milestones.items():
        if type(milestone.get("number")) is not int or milestone["number"] <= 0:
            raise RoadmapError(f"GitHub milestone {title!r} has malformed number metadata")
    managed_labels = {label["name"] for label in program["labels"]}
    tasks_by_id = {task.task_id: task for task in tasks}
    title_to_task = {task.title: task.task_id for task in tasks}
    operations: list[dict[str, Any]] = []
    for label in program["labels"]:
        current = current_labels.get(label["name"])
        if current is None:
            operations.append({"action": "create_label", "blocking": False, "name": label["name"], "payload": label})
        elif current.get("color", "").lower() != label["color"].lower() or (
            current.get("description") or ""
        ) != label["description"]:
            operations.append(
                _blocking(
                    "label_definition_drift",
                    name=label["name"],
                    expected={
                        "color": label["color"].lower(),
                        "description": label["description"],
                    },
                    actual={
                        "color": current.get("color", "").lower(),
                        "description": current.get("description") or "",
                    },
                )
            )
    for milestone in program["milestones"]:
        current = current_milestones.get(milestone["title"])
        payload = {"title": milestone["title"], "description": milestone["description"], "due_on": milestone["due_on"]}
        if current is None:
            operations.append(
                {"action": "create_milestone", "blocking": False, "title": milestone["title"], "payload": payload}
            )
        else:
            current_due_on = current.get("due_on")
            due_date_matches = current_due_on is not None and _parse_due_on(
                current_due_on, f"GitHub milestone {milestone['title']!r}.due_on"
            ).date() == _parse_due_on(
                milestone["due_on"], f"program milestone {milestone['title']!r}.due_on"
            ).date()
            definition_matches = (
                (current.get("description") or "") == milestone["description"]
                and due_date_matches
            )
            if definition_matches:
                continue
            operations.append(
                _blocking(
                    "milestone_definition_drift",
                    title=milestone["title"],
                    expected={
                        "description": milestone["description"],
                        "due_on": milestone["due_on"],
                    },
                    actual={
                        "description": current.get("description") or "",
                        "due_on": current.get("due_on") or "",
                    },
                )
            )

    marker_to_issue: dict[str, int] = {}
    for issue_number, issue in sorted(current_issues.items()):
        _validate_issue_fields(issue_number, issue)
        body = issue.get("body") or ""
        markers = _issue_markers(body)
        marker_ids = [task_id for task_id, _ in markers]
        title_task_id = title_to_task.get(issue["title"])
        sanitized_body = sanitize_public_issue_body(body)
        body_needs_sanitization = sanitized_body != body
        if (markers or title_task_id is not None) and body_needs_sanitization:
            operations.append(
                {
                    "action": "sanitize_issue_body",
                    "blocking": False,
                    "task": title_task_id or (marker_ids[0] if marker_ids else None),
                    "number": issue_number,
                    "body": sanitized_body,
                }
            )
        declaration_count = len(MARKER_HINT_RE.findall(body))
        if declaration_count != len(markers):
            operations.append(
                _blocking(
                    "invalid_marker",
                    number=issue_number,
                    declarations=declaration_count,
                    parsed=len(markers),
                )
            )
        if len(markers) > 1:
            operations.append(
                _blocking(
                    "duplicate_identity",
                    number=issue_number,
                    ids=sorted(marker_ids),
                )
            )
        if not markers:
            if title_task_id is None:
                continue
            previous = marker_to_issue.get(title_task_id)
            if previous is not None and previous != issue_number:
                operations.append(
                    _blocking(
                        "duplicate_identity",
                        task=title_task_id,
                        numbers=[previous, issue_number],
                    )
                )
            marker_to_issue[title_task_id] = issue_number
            mapped_issue = mapping.get(title_task_id)
            if mapped_issue is not None and mapped_issue != issue_number:
                operations.append(
                    _blocking(
                        "mapping_conflict",
                        task=title_task_id,
                        mapped_issue=mapped_issue,
                        title_issue=issue_number,
                    )
                )
            continue
        task_id, sync_hash = markers[0]
        if task_id not in tasks_by_id:
            operations.append(_blocking("stale_marker", number=issue_number, task=task_id))
            continue
        expected_sync_hash = forge_task_sync_hash(tasks_by_id[task_id])
        if sync_hash is None:
            if not (body_needs_sanitization and title_task_id == task_id):
                operations.append(
                    _blocking("marker_sync_missing", number=issue_number, task=task_id)
                )
        elif sync_hash != expected_sync_hash:
            if not (body_needs_sanitization and title_task_id == task_id):
                operations.append(
                    _blocking(
                        "marker_sync_conflict",
                        number=issue_number,
                        task=task_id,
                        expected=expected_sync_hash,
                        actual=sync_hash,
                    )
                )
        previous = marker_to_issue.get(task_id)
        if previous is not None and previous != issue_number:
            operations.append(_blocking("duplicate_identity", task=task_id, numbers=[previous, issue_number]))
        marker_to_issue[task_id] = issue_number
        mapped_issue = mapping.get(task_id)
        if mapped_issue is not None and mapped_issue != issue_number:
            operations.append(
                _blocking("mapping_conflict", task=task_id, mapped_issue=mapped_issue, marker_issue=issue_number)
            )

    owner = program["owner"]
    deferred_tasks: list[str] = []
    for task in tasks:
        issue_number = mapping.get(task.task_id) or marker_to_issue.get(task.task_id)
        if issue_number is None:
            operations.append(_blocking("missing_mapping", task=task.task_id))
            continue
        issue = current_issues.get(issue_number)
        if issue is None:
            operations.append(_blocking("missing_issue", task=task.task_id, number=issue_number))
            continue
        markers = _issue_markers(issue.get("body"))
        marker_ids = [task_id for task_id, _ in markers]
        title_matches = issue.get("title") == task.title
        if (len(markers) != 1 or markers[0][0] != task.task_id) and not (
            not markers and title_matches
        ):
            operations.append(
                _blocking(
                    "marker_conflict" if markers else "marker_missing",
                    task=task.task_id,
                    number=issue_number,
                    marker_ids=sorted(marker_ids),
                )
            )
        milestone = current_milestones.get(task.release)
        if milestone is None:
            deferred_tasks.append(task.task_id)
            continue
        current_names = {item["name"] for item in issue.get("labels", [])}
        current_managed = {
            name for name in current_names if is_managed_label(name, managed_labels)
        }
        desired_managed = set(desired_labels(task))
        for name in sorted(desired_managed - current_managed):
            operations.append(
                {
                    "action": "add_issue_label",
                    "blocking": False,
                    "task": task.task_id,
                    "number": issue_number,
                    "name": name,
                }
            )
        for name in sorted(current_managed - desired_managed):
            operations.append(
                {
                    "action": "remove_issue_label",
                    "blocking": False,
                    "task": task.task_id,
                    "number": issue_number,
                    "name": name,
                }
            )
        current_assignees = [item["login"] for item in issue.get("assignees", [])]
        owner_present = any(
            login.casefold() == owner.casefold() for login in current_assignees
        )
        owner_required = task.status in OWNER_MANAGED_STATUSES
        if owner_required and not owner_present:
            operations.append(
                {
                    "action": "add_owner_assignee",
                    "blocking": False,
                    "task": task.task_id,
                    "number": issue_number,
                    "owner": owner,
                }
            )
        elif owner_present and not owner_required:
            operations.append(
                {
                    "action": "remove_owner_assignee",
                    "blocking": False,
                    "task": task.task_id,
                    "number": issue_number,
                    "owner": owner,
                }
            )
        current_milestone = (issue.get("milestone") or {}).get("number")
        if current_milestone != milestone["number"]:
            operations.append(
                {
                    "action": "set_issue_milestone",
                    "blocking": False,
                    "task": task.task_id,
                    "number": issue_number,
                    "milestone": int(milestone["number"]),
                }
            )
    if deferred_tasks:
        operations.append(
            {
                "action": "defer_issue_metadata",
                "blocking": False,
                "tasks": deferred_tasks,
                "reason": "milestones must be created before their issue numbers are known",
            }
        )
    if forge_plan_digest is None:
        operations.append(
            _blocking(
                "forge_native_parity_required",
                scope=list(SYNC_SCOPE),
                message=(
                    "Provide an inspected clean Forge plan before synchronizing metadata; "
                    "issue title/body/state/sub-issue/blocked-by parity is Forge-owned."
                ),
            )
        )
    return operations


def compute_plan_digest(
    repository: str,
    operations: list[dict[str, Any]],
    forge_plan_digest: str | None = None,
) -> str:
    if forge_plan_digest is not None and not re.fullmatch(r"[0-9a-f]{64}", forge_plan_digest):
        raise RoadmapError("Forge parity plan digest must be a SHA-256 digest")
    ordered_operations = sorted(
        operations,
        key=lambda operation: json.dumps(operation, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
    )
    document = {
        "schema_version": SYNC_SCHEMA_VERSION,
        "repository": _validate_repository(repository),
        "scope": list(SYNC_SCOPE),
        "forge_plan_digest": forge_plan_digest,
        "operations": ordered_operations,
    }
    encoded = json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


plan_digest = compute_plan_digest


def plan_blockers(operations: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [operation for operation in operations if operation.get("blocking") is True]


def apply_metadata(
    program: dict[str, Any],
    tasks: list[Task],
    repository: str,
    inspected_plan_digest: str | None = None,
    forge_plan_digest: str | None = None,
    forge_tasks_path: Path | None = None,
) -> dict[str, Any]:
    _configured_repository(program, repository)
    validate(program, tasks)
    if inspected_plan_digest is None or not re.fullmatch(r"[0-9a-f]{64}", inspected_plan_digest):
        raise RoadmapError("github-apply requires an inspected 64-character plan digest")
    if forge_plan_digest is None or not re.fullmatch(r"[0-9a-f]{64}", forge_plan_digest):
        raise RoadmapError("github-apply requires an inspected clean Forge parity plan")
    if forge_tasks_path is None:
        raise RoadmapError(
            "github-apply requires the Forge task-ledger script for a live parity check"
        )
    github = GitHub(repository)
    prefix = f"repos/{repository}"
    counts: dict[str, int] = {}
    first = metadata_plan(program, tasks, repository, forge_plan_digest)
    actual_digest = compute_plan_digest(repository, first, forge_plan_digest)
    if actual_digest != inspected_plan_digest:
        raise RoadmapError(
            f"plan digest mismatch: inspected {inspected_plan_digest}, current preflight is {actual_digest}"
        )
    blockers = plan_blockers(first)
    if blockers:
        raise RoadmapError(f"metadata synchronization blockers: {json.dumps(blockers[:20], sort_keys=True)}")
    verify_local_snapshot_current(program, tasks)
    live_forge_plan_digest = verify_live_forge_parity(
        forge_tasks_path,
        repository,
    )
    verify_local_snapshot_current(program, tasks)
    current = metadata_plan(program, tasks, repository, forge_plan_digest)
    current_digest = compute_plan_digest(repository, current, forge_plan_digest)
    if current_digest != inspected_plan_digest:
        raise RoadmapError(
            "GitHub metadata changed during the live Forge parity check; "
            "inspect a new plan"
        )
    current_blockers = plan_blockers(current)
    if current_blockers:
        raise RoadmapError(
            "metadata synchronization blockers after live Forge parity check: "
            f"{json.dumps(current_blockers[:20], sort_keys=True)}"
        )
    for operation in current:
        action = operation["action"]
        if action == "create_label":
            github.request("POST", f"{prefix}/labels", operation["payload"])
        elif action == "create_milestone":
            github.request("POST", f"{prefix}/milestones", operation["payload"])
        elif action == "add_issue_label":
            github.request(
                "POST",
                f"{prefix}/issues/{operation['number']}/labels",
                {"labels": [operation["name"]]},
            )
        elif action == "remove_issue_label":
            name = urllib.parse.quote(operation["name"], safe="")
            github.request(
                "DELETE",
                f"{prefix}/issues/{operation['number']}/labels/{name}",
            )
        elif action == "add_owner_assignee":
            github.request(
                "POST",
                f"{prefix}/issues/{operation['number']}/assignees",
                {"assignees": [operation["owner"]]},
            )
        elif action == "remove_owner_assignee":
            github.request(
                "DELETE",
                f"{prefix}/issues/{operation['number']}/assignees",
                {"assignees": [operation["owner"]]},
            )
        elif action == "set_issue_milestone":
            github.request(
                "PATCH",
                f"{prefix}/issues/{operation['number']}",
                {"milestone": operation["milestone"]},
            )
        elif action == "sanitize_issue_body":
            github.request(
                "PATCH",
                f"{prefix}/issues/{operation['number']}",
                {"body": operation["body"]},
            )
        elif action == "defer_issue_metadata":
            continue
        else:
            raise RoadmapError(f"unsupported metadata operation: {action}")
        counts[action] = counts.get(action, 0) + 1
        time.sleep(MUTATION_INTERVAL_SECONDS)
    remaining = metadata_plan(program, tasks, repository, forge_plan_digest)
    remaining_blockers = plan_blockers(remaining)
    if remaining_blockers:
        raise RoadmapError(
            "metadata synchronization blockers after apply: "
            f"{json.dumps(remaining_blockers[:20], sort_keys=True)}"
        )
    return {
        "counts": counts,
        "live_forge_plan_digest": live_forge_plan_digest,
        "remaining_operations": len(remaining),
        "remaining_blockers": 0,
        "next_plan_digest": compute_plan_digest(repository, remaining, forge_plan_digest),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    subparsers.add_parser("task-digest")
    index = subparsers.add_parser("index")
    index.add_argument("--write", action="store_true")
    github_plan = subparsers.add_parser("github-plan")
    github_plan.add_argument("--repo")
    github_plan.add_argument("--forge-plan", type=Path, required=True)
    github_apply = subparsers.add_parser("github-apply")
    github_apply.add_argument("--repo")
    github_apply.add_argument("--yes", action="store_true")
    github_apply.add_argument("--plan-digest", required=True)
    github_apply.add_argument("--forge-plan", type=Path, required=True)
    github_apply.add_argument("--forge-tasks", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        program = load_program()
        tasks = load_tasks()
        if args.command == "check":
            print(json.dumps(validate(program, tasks), indent=2, sort_keys=True))
        elif args.command == "task-digest":
            validate(program, tasks)
            print(compute_task_tree_digest(tasks))
        elif args.command == "index":
            content = render_index(tasks)
            if args.write:
                index_path = TASKS_DIR / "README.md"
                _strict_regular(index_path, "task index")
                index_path.write_text(content, encoding="utf-8")
                print(json.dumps({"written": str(TASKS_DIR / "README.md"), "tasks": len(tasks)}))
            else:
                sys.stdout.write(content)
        elif args.command == "github-plan":
            repository = args.repo or program["repository"]
            task_tree_digest = compute_task_tree_digest(tasks)
            forge_plan_digest = load_forge_parity_plan(
                args.forge_plan,
                repository,
                task_tree_digest,
            )
            operations = metadata_plan(program, tasks, repository, forge_plan_digest)
            print(
                json.dumps(
                    {
                        "repository": repository,
                        "scope": list(SYNC_SCOPE),
                        "task_tree_digest": task_tree_digest,
                        "forge_plan_digest": forge_plan_digest,
                        "plan_digest": compute_plan_digest(
                            repository, operations, forge_plan_digest
                        ),
                        "operations": operations,
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
        elif args.command == "github-apply":
            if not args.yes:
                raise RoadmapError("github-apply requires --yes")
            repository = args.repo or program["repository"]
            task_tree_digest = compute_task_tree_digest(tasks)
            forge_plan_digest = load_forge_parity_plan(
                args.forge_plan,
                repository,
                task_tree_digest,
            )
            print(
                json.dumps(
                    {
                        "repository": repository,
                        "task_tree_digest": task_tree_digest,
                        "forge_plan_digest": forge_plan_digest,
                        "applied": apply_metadata(
                            program,
                            tasks,
                            repository,
                            args.plan_digest,
                            forge_plan_digest,
                            args.forge_tasks,
                        ),
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
    except (OSError, json.JSONDecodeError, subprocess.TimeoutExpired, RoadmapError) as exc:
        print(f"roadmap: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
