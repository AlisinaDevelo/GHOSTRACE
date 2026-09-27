#!/usr/bin/env python3
"""Check shipped artifacts against the reviewed permission manifest.

`planning/permission-manifest.json` lists every artifact with its bundle,
helpers, extensions, signature class, hardened-runtime and sandbox state,
entitlements, linked libraries, privacy-sensitive APIs, filesystem rights, and
network capability. `approved_permission_digest` binds that permission set to
its privacy, threat, test, and migration review: any new or broadened
permission changes the digest and fails until the review is updated.

With `--binary`, the signed entitlements (`codesign`) and linked libraries
(`otool -L`) of a built macOS artifact are extracted and compared with the
manifest. Forbidden entitlements and network framework linkage fail even if a
manifest lists them. Uses only the standard library.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import plistlib
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "planning" / "permission-manifest.json"

ARTIFACT_FIELDS = {
    "id",
    "kind",
    "path",
    "bundle_identifier",
    "helpers",
    "extensions",
    "code_signature",
    "hardened_runtime",
    "sandboxed",
    "entitlements",
    "linked_libraries",
    "privacy_sensitive_apis",
    "filesystem_rights",
    "network_capability",
}
# Fields whose change alters what the artifact is permitted to do.
PERMISSION_FIELDS = sorted(ARTIFACT_FIELDS - {"path"})
REVIEW_FIELDS = {"approved_permission_digest", "privacy", "threat_model", "tests", "migration"}
SIGNATURE_CLASSES = {"adhoc_linker", "developer_id", "unsigned"}


class ManifestError(ValueError):
    """The manifest or an artifact violates the reviewed permission contract."""


def load(path: Path = MANIFEST_PATH) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ManifestError(f"cannot read {path.name}") from exc
    return document


def permission_digest(document: dict[str, Any]) -> str:
    """Digest of every permission-relevant field, independent of key order."""
    canonical = {
        "forbidden_entitlements": sorted(document["forbidden_entitlements"]),
        "network_linkage_markers": sorted(document["network_linkage_markers"]),
        "artifacts": [
            {field: artifact[field] for field in PERMISSION_FIELDS}
            for artifact in sorted(document["artifacts"], key=lambda item: item["id"])
        ],
    }
    encoded = json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(b"ghostrace-permission-manifest-v1\0" + encoded).hexdigest()


def validate(document: Any, root: Path = ROOT) -> dict[str, Any]:
    if type(document) is not dict or set(document) != {
        "schema_version",
        "forbidden_entitlements",
        "network_linkage_markers",
        "artifacts",
        "review",
    }:
        raise ManifestError("permission manifest has an invalid schema")
    if document["schema_version"] != 1:
        raise ManifestError("unsupported permission manifest version")
    forbidden = document["forbidden_entitlements"]
    if type(forbidden) is not list or not forbidden or any(type(item) is not str for item in forbidden):
        raise ManifestError("forbidden_entitlements must be a non-empty string list")
    artifacts = document["artifacts"]
    if type(artifacts) is not list or not artifacts:
        raise ManifestError("artifacts must be a non-empty list")
    seen: set[str] = set()
    for artifact in artifacts:
        if type(artifact) is not dict or set(artifact) != ARTIFACT_FIELDS:
            raise ManifestError("artifact entry has an invalid schema")
        if artifact["id"] in seen:
            raise ManifestError(f"duplicate artifact {artifact['id']}")
        seen.add(artifact["id"])
        if artifact["code_signature"] not in SIGNATURE_CLASSES:
            raise ManifestError(f"{artifact['id']}: unknown signature class")
        if type(artifact["entitlements"]) is not dict:
            raise ManifestError(f"{artifact['id']}: entitlements must be an object")
        listed_forbidden = sorted(set(artifact["entitlements"]) & set(forbidden))
        if listed_forbidden:
            raise ManifestError(
                f"{artifact['id']}: forbidden entitlement in manifest: {', '.join(listed_forbidden)}"
            )
        if artifact["network_capability"] != "none":
            raise ManifestError(f"{artifact['id']}: network capability requires a new review")
        if artifact["code_signature"] != "unsigned" and artifact["kind"] == "app_bundle" and not artifact["hardened_runtime"]:
            raise ManifestError(f"{artifact['id']}: signed app bundles require the hardened runtime")
    review = document["review"]
    if type(review) is not dict or set(review) != REVIEW_FIELDS:
        raise ManifestError("review has an invalid schema")
    for field in ("privacy", "threat_model"):
        if not (root / review[field]).is_file():
            raise ManifestError(f"review {field} evidence is missing")
    if type(review["tests"]) is not list or not review["tests"]:
        raise ManifestError("review must name tests")
    for test in review["tests"]:
        if not (root / test).is_file():
            raise ManifestError(f"review test {test} is missing")
    if type(review["migration"]) is not str or not review["migration"].strip():
        raise ManifestError("review must state migration impact")
    digest = permission_digest(document)
    if review["approved_permission_digest"] != digest:
        raise ManifestError(
            "permission set changed without review: update privacy, threat, test, and "
            f"migration evidence, then set approved_permission_digest to {digest}"
        )
    return {"artifacts": len(artifacts), "permission_digest": digest, "ok": True}


def extract_entitlements(binary: Path) -> dict[str, Any]:
    result = subprocess.run(
        ["codesign", "-d", "--entitlements", "-", "--xml", str(binary)],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise ManifestError("codesign could not read the artifact signature")
    payload = result.stdout.strip()
    if not payload:
        return {}
    try:
        entitlements = plistlib.loads(payload)
    except Exception as exc:  # plistlib raises several unrelated types
        raise ManifestError("artifact entitlements are not a valid plist") from exc
    if type(entitlements) is not dict:
        raise ManifestError("artifact entitlements are not a dictionary")
    return entitlements


def extract_signature_flags(binary: Path) -> str:
    result = subprocess.run(
        ["codesign", "-dv", str(binary)], capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise ManifestError("codesign could not read the artifact signature")
    for line in result.stderr.splitlines():
        if line.startswith("CodeDirectory "):
            return line
    raise ManifestError("artifact has no code directory")


def extract_linked_libraries(binary: Path) -> list[str]:
    result = subprocess.run(["otool", "-L", str(binary)], capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise ManifestError("otool could not read the artifact load commands")
    libraries = []
    for line in result.stdout.splitlines()[1:]:
        line = line.strip()
        if line:
            libraries.append(line.split(" (compatibility", 1)[0])
    return sorted(libraries)


def compare_artifact(
    document: dict[str, Any],
    artifact_id: str,
    entitlements: dict[str, Any],
    linked_libraries: list[str],
    signature_flags: str,
) -> list[str]:
    """Return every drift between the observed artifact and its manifest entry."""
    artifact = next((item for item in document["artifacts"] if item["id"] == artifact_id), None)
    if artifact is None:
        return [f"artifact {artifact_id} is not in the reviewed manifest"]
    problems = []
    forbidden = sorted(set(entitlements) & set(document["forbidden_entitlements"]))
    if forbidden:
        problems.append(f"forbidden entitlement present: {', '.join(forbidden)}")
    added = sorted(set(entitlements) - set(artifact["entitlements"]))
    if added:
        problems.append(f"entitlement not in the reviewed manifest: {', '.join(added)}")
    changed = sorted(
        key
        for key in set(entitlements) & set(artifact["entitlements"])
        if entitlements[key] != artifact["entitlements"][key]
    )
    if changed:
        problems.append(f"entitlement value differs from review: {', '.join(changed)}")
    network = sorted(
        library
        for library in linked_libraries
        if any(marker in library for marker in document["network_linkage_markers"])
    )
    if network:
        problems.append(f"network library linked: {', '.join(network)}")
    unexpected = sorted(set(linked_libraries) - set(artifact["linked_libraries"]))
    if unexpected:
        problems.append(f"library not in the reviewed manifest: {', '.join(unexpected)}")
    if "get-task-allow" in signature_flags:
        problems.append("signature allows task-port debugging")
    runtime = "runtime" in signature_flags
    if runtime != artifact["hardened_runtime"]:
        problems.append("hardened runtime state differs from review")
    if artifact["code_signature"] == "adhoc_linker" and "adhoc" not in signature_flags:
        problems.append("signature class differs from review")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    check = sub.add_parser("check")
    check.add_argument("--binary", type=Path, help="built artifact to inspect (macOS only)")
    check.add_argument("--artifact", default="ghostrace-cli")
    sub.add_parser("digest")
    args = parser.parse_args(argv)
    try:
        document = load()
        if args.command == "digest":
            print(permission_digest(document))
            return 0
        report = validate(document)
        if args.binary is not None:
            if sys.platform != "darwin":
                raise ManifestError("artifact inspection requires macOS codesign and otool")
            problems = compare_artifact(
                document,
                args.artifact,
                extract_entitlements(args.binary),
                extract_linked_libraries(args.binary),
                extract_signature_flags(args.binary),
            )
            if problems:
                raise ManifestError("; ".join(problems))
            report["artifact_checked"] = args.artifact
    except ManifestError as exc:
        print(f"permissions: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
