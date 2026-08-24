#!/usr/bin/env python3
"""Check the pinned developer inputs without contacting a service."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "toolchain" / "manifest.json"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


class ReproducibilityError(ValueError):
    """A pinned input or reproducibility check is invalid."""


def _json(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as exc:
        raise ReproducibilityError(f"cannot read {path.relative_to(ROOT)}") from exc


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise ReproducibilityError(f"cannot read {path.relative_to(ROOT)}") from exc
    return digest.hexdigest()


def _check_file_digest(document: dict[str, Any], field: str) -> None:
    if type(document) is not dict or set(document) != {"path", "sha256"}:
        raise ReproducibilityError(f"{field} has an invalid schema")
    path_value = document["path"]
    if type(path_value) is not str or not path_value or Path(path_value).is_absolute():
        raise ReproducibilityError(f"{field}.path must be relative")
    path = (ROOT / path_value).resolve()
    if ROOT not in path.parents or not path.is_file():
        raise ReproducibilityError(f"{field}.path is not a repository file")
    digest = document["sha256"]
    if type(digest) is not str or SHA256_RE.fullmatch(digest) is None:
        raise ReproducibilityError(f"{field}.sha256 is not a lowercase SHA-256 digest")
    if _sha256(path) != digest:
        raise ReproducibilityError(f"{field}.sha256 does not match {path_value}")


def validate() -> dict[str, Any]:
    document = _json(MANIFEST_PATH)
    required = {"schema_version", "rust", "python", "dependency_policy", "github_actions", "fixtures"}
    if type(document) is not dict or set(document) != required:
        raise ReproducibilityError("toolchain manifest has an invalid schema")
    if document["schema_version"] != 1:
        raise ReproducibilityError("unsupported toolchain manifest schema")

    rust = document["rust"]
    if type(rust) is not dict or rust.get("channel") != "1.88.0":
        raise ReproducibilityError("Rust channel must remain pinned to 1.88.0")
    if rust.get("source") != "rustup official channel manifest":
        raise ReproducibilityError("Rust install source is not pinned")
    if rust.get("profile") != "minimal" or rust.get("components") != ["clippy", "rustfmt"]:
        raise ReproducibilityError("Rust profile/components drifted")
    _check_file_digest(rust.get("rust_toolchain_file"), "rust.rust_toolchain_file")
    _check_file_digest(rust.get("cargo_lock"), "rust.cargo_lock")
    toolchain = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    if 'channel = "1.88.0"' not in toolchain or 'profile = "minimal"' not in toolchain:
        raise ReproducibilityError("rust-toolchain.toml does not match the manifest")
    for component in ("clippy", "rustfmt"):
        if component not in toolchain:
            raise ReproducibilityError(f"rust-toolchain.toml is missing {component}")

    python = document["python"]
    if type(python) is not dict or python.get("command") != "python3":
        raise ReproducibilityError("Python command must remain python3")
    if python.get("minimum_version") != "3.9" or python.get("dependencies") != "standard-library-only":
        raise ReproducibilityError("Python reproducibility policy drifted")
    if sys.version_info < (3, 9):
        raise ReproducibilityError("Python 3.9 or newer is required")

    policy = document["dependency_policy"]
    if type(policy) is not dict or policy.get("cargo_lock_required") is not True:
        raise ReproducibilityError("Cargo.lock is required")
    if policy.get("cargo_commands_require_locked") is not True:
        raise ReproducibilityError("Cargo commands must use --locked")
    if policy.get("offline_smoke_sets") != "CARGO_NET_OFFLINE=true":
        raise ReproducibilityError("offline smoke policy drifted")
    if policy.get("network_clients_in_product") is not False:
        raise ReproducibilityError("product network policy drifted")

    actions = document["github_actions"]
    if type(actions) is not dict or actions.get("workflow_toolchain") != "1.88.0":
        raise ReproducibilityError("workflow toolchain is not pinned")
    workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    if "toolchain: stable" in workflow or "toolchain: 'stable'" in workflow:
        raise ReproducibilityError("CI still requests the floating stable toolchain")
    if workflow.count("toolchain: 1.88.0") < 3:
        raise ReproducibilityError("CI does not pin every Rust test matrix entry")
    for action_field in ("checkout", "rust_toolchain"):
        action_ref = actions.get(action_field)
        if type(action_ref) is not str or action_ref not in workflow:
            raise ReproducibilityError(f"CI action pin {action_field} is not present")

    fixture_check = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "fixture-manifest.py"), "check"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if fixture_check.returncode:
        raise ReproducibilityError(fixture_check.stderr.strip() or "fixture manifest check failed")
    fixture_result = json.loads(fixture_check.stdout)
    if fixture_result.get("ok") is not True:
        raise ReproducibilityError("fixture manifest did not report success")

    return {
        "cargo_lock_sha256": rust["cargo_lock"]["sha256"],
        "fixtures": fixture_result["fixtures"],
        "python": f"{sys.version_info.major}.{sys.version_info.minor}",
        "rust_channel": rust["channel"],
        "ok": True,
    }


def main() -> int:
    if sys.argv[1:] != ["check"]:
        print("usage: python3 scripts/reproducibility.py check", file=sys.stderr)
        return 2
    try:
        print(json.dumps(validate(), sort_keys=True))
    except (OSError, ReproducibilityError, json.JSONDecodeError) as exc:
        print(f"reproducibility: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
