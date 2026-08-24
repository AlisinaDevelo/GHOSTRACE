#!/usr/bin/env python3
"""Validate the synthetic fixture corpus and its reproducibility metadata.

The checker deliberately uses only the Python standard library.  Fixture files
are immutable inputs: this command hashes them and compares the recorded byte
length, digest, schema, and deterministic generator metadata without writing to
the repository.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "fixtures" / "manifest.json"
GENERATOR_VERSION = "ghostrace-fixture-manifest-v1"
DEFAULT_SEED = "ghostrace-fixture-seed-v1"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


class ManifestError(ValueError):
    """A malformed or stale fixture manifest."""


def _read_json(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as exc:
        raise ManifestError(f"cannot read {path.relative_to(ROOT)}") from exc


def _require_string(value: Any, field: str) -> str:
    if type(value) is not str or not value:
        raise ManifestError(f"{field} must be a non-empty string")
    return value


def _require_sha256(value: Any, field: str) -> str:
    value = _require_string(value, field)
    if SHA256_RE.fullmatch(value) is None:
        raise ManifestError(f"{field} must be a lowercase SHA-256 digest")
    return value


def _relative_path(value: Any, field: str) -> Path:
    raw = _require_string(value, field)
    path = Path(raw)
    if path.is_absolute() or ".." in path.parts or path == Path("."):
        raise ManifestError(f"{field} must be a repository-relative file path")
    resolved = (ROOT / path).resolve()
    if resolved.parent == ROOT / ".git" or ROOT not in resolved.parents:
        raise ManifestError(f"{field} escapes the repository")
    return path


def _hash_file(path: Path) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                digest.update(chunk)
    except OSError as exc:
        raise ManifestError(f"cannot read fixture {path.relative_to(ROOT)}") from exc
    return size, digest.hexdigest()


def validate_manifest() -> dict[str, Any]:
    document = _read_json(MANIFEST_PATH)
    if type(document) is not dict:
        raise ManifestError("manifest must be an object")
    required = {"schema_version", "generator", "fixtures", "privacy"}
    if set(document) != required:
        raise ManifestError(f"manifest keys must be {sorted(required)}")
    if document["schema_version"] != 1:
        raise ManifestError("unsupported fixture manifest schema")

    generator = document["generator"]
    if type(generator) is not dict or set(generator) != {
        "version",
        "seed",
        "algorithm",
        "command",
    }:
        raise ManifestError("generator metadata has an invalid schema")
    if generator["version"] != GENERATOR_VERSION:
        raise ManifestError("generator.version does not match the checker")
    if generator["seed"] != DEFAULT_SEED:
        raise ManifestError("generator.seed does not match the pinned seed")
    if generator["algorithm"] != "sha256(seed) for manifest binding; no runtime randomness":
        raise ManifestError("generator.algorithm is not the documented deterministic algorithm")
    if generator["command"] != "python3 scripts/fixture-manifest.py check":
        raise ManifestError("generator.command is not the clean-machine checker")

    privacy = document["privacy"]
    if type(privacy) is not dict or set(privacy) != {
        "synthetic_only",
        "user_data_included",
        "network_required",
    }:
        raise ManifestError("privacy metadata has an invalid schema")
    if privacy != {
        "synthetic_only": True,
        "user_data_included": False,
        "network_required": False,
    }:
        raise ManifestError("fixture privacy metadata must remain synthetic and offline")

    fixtures = document["fixtures"]
    if type(fixtures) is not list or not fixtures:
        raise ManifestError("fixtures must be a non-empty list")
    seen: set[str] = set()
    for index, entry in enumerate(fixtures):
        field = f"fixtures[{index}]"
        if type(entry) is not dict or set(entry) != {
            "path",
            "format",
            "generator_version",
            "seed",
            "sha256",
            "bytes",
        }:
            raise ManifestError(f"{field} has an invalid schema")
        relative = _relative_path(entry["path"], f"{field}.path")
        if relative.as_posix() in seen:
            raise ManifestError(f"duplicate fixture path {relative}")
        seen.add(relative.as_posix())
        if entry["format"] not in {"json", "jsonl"}:
            raise ManifestError(f"{field}.format must be json or jsonl")
        if entry["generator_version"] != GENERATOR_VERSION:
            raise ManifestError(f"{field}.generator_version is not pinned")
        if entry["seed"] != DEFAULT_SEED:
            raise ManifestError(f"{field}.seed is not pinned")
        expected_size = entry["bytes"]
        if type(expected_size) is not int or expected_size <= 0:
            raise ManifestError(f"{field}.bytes must be a positive integer")
        expected_digest = _require_sha256(entry["sha256"], f"{field}.sha256")
        path = ROOT / relative
        if not path.is_file():
            raise ManifestError(f"fixture does not exist: {relative}")
        actual_size, actual_digest = _hash_file(path)
        if actual_size != expected_size:
            raise ManifestError(
                f"{relative} byte length drifted: expected {expected_size}, got {actual_size}"
            )
        if actual_digest != expected_digest:
            raise ManifestError(
                f"{relative} digest drifted: expected {expected_digest}, got {actual_digest}"
            )

    return {
        "fixtures": len(fixtures),
        "generator_version": generator["version"],
        "seed": generator["seed"],
        "ok": True,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check",))
    args = parser.parse_args(argv)
    try:
        result = validate_manifest()
    except ManifestError as exc:
        print(f"fixture-manifest: {exc}", file=sys.stderr)
        return 1
    if args.command == "check":
        print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
