#!/usr/bin/env python3
"""Run the isolated macOS Keychain lifecycle probe and emit its matrix.

The probe mutates only a temporary keychain created by the ignored Rust test.
Screen sleep, user switching, logout, and a real GHOSTRACE launchd helper are
not triggered automatically: the matrix records those explicit no-go limits
instead of treating a hosted or synthetic run as device evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)


def output(command: list[str], fallback: str) -> str:
    result = run(command)
    return result.stdout.strip() or fallback


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def device() -> dict[str, str]:
    os_name = output(["/usr/bin/sw_vers", "-productName"], platform.system())
    os_version = output(["/usr/bin/sw_vers", "-productVersion"], platform.release())
    os_build = output(["/usr/bin/sw_vers", "-buildVersion"], "unknown")
    model = output(["/usr/sbin/sysctl", "-n", "hw.model"], "unknown")
    architecture = "arm64" if platform.machine() in {"arm64", "aarch64"} else platform.machine()
    rust = output(["rustc", "+1.88.0", "-Vv"], "rustc 1.88.0")
    rust_line = rust.splitlines()[0] if rust else "rustc 1.88.0"
    return {
        "model": model,
        "os": f"{os_name} {os_version}",
        "os_build": os_build,
        "architecture": architecture,
        "toolchain": rust_line,
    }


def transition_rows(probe_passed: bool) -> list[dict[str, str]]:
    if probe_passed:
        login = {
            "name": "login/unlocked",
            "status": "observed",
            "keychain_availability": "available",
            "prompt_behavior": "not-requested",
            "buffer_behavior": "commit",
            "evidence": "The isolated provider read the provisioned key before lock.",
            "limitation": "This is the current logged-in, unlocked user session.",
        }
        lock = {
            "name": "lock",
            "status": "observed",
            "keychain_availability": "unavailable",
            "prompt_behavior": "not-requested",
            "buffer_behavior": "emit-gap",
            "evidence": "The isolated keychain was locked; the provider failed closed and Writer emitted KeyUnavailable.",
            "limitation": "The probe locks an isolated Keychain, not the display lock or another user's session.",
        }
    else:
        login = {
            "name": "login/unlocked",
            "status": "no-go",
            "keychain_availability": "not-exercised",
            "prompt_behavior": "prompt-not-observed",
            "buffer_behavior": "not-exercised",
            "evidence": "The isolated Keychain probe did not pass.",
            "limitation": "Resolve the device probe failure before making a positive lifecycle claim.",
        }
        lock = {
            "name": "lock",
            "status": "no-go",
            "keychain_availability": "not-exercised",
            "prompt_behavior": "prompt-not-observed",
            "buffer_behavior": "not-exercised",
            "evidence": "The isolated Keychain probe did not pass.",
            "limitation": "Resolve the device probe failure before making a positive lifecycle claim.",
        }
    no_go = [
        (
            "sleep",
            "Screen sleep is not triggered automatically because it suspends the active device session.",
        ),
        (
            "wake",
            "Wake requires a preceding interactive sleep transition and is not triggered automatically.",
        ),
        (
            "fast-user-switch",
            "No second authorized interactive user session is available for this run.",
        ),
        (
            "logout",
            "Logout would terminate the harness and cannot be safely automated in this run.",
        ),
        (
            "launchd-restart",
            "No GHOSTRACE launchd helper is enabled; restarting an unrelated user service is out of scope.",
        ),
    ]
    rows = [login, lock]
    rows.extend(
        {
            "name": name,
            "status": "no-go",
            "keychain_availability": "not-exercised",
            "prompt_behavior": "interactive-required",
            "buffer_behavior": "not-exercised",
            "evidence": "No positive claim is made; this transition is recorded as an explicit no-go.",
            "limitation": limitation,
        }
        for name, limitation in no_go
    )
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args(argv)

    if platform.system() != "Darwin":
        print("key-availability-matrix: macOS is required", file=sys.stderr)
        return 2
    command = [
        "cargo",
        "+1.88.0",
        "test",
        "--locked",
        "--offline",
        "--test",
        "keychain_lifecycle",
        "--",
        "--ignored",
        "--nocapture",
    ]
    result = run(command)
    args.log.write_text(
        f"COMMAND: {' '.join(command)}\nEXIT: {result.returncode}\n\n"
        f"STDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}",
        encoding="utf-8",
    )
    probe_passed = result.returncode == 0
    document = {
        "schema_version": 1,
        "contract_id": "ghostrace-key-availability-matrix-v1",
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "source_commit": args.source_commit,
        "device": device(),
        "probe": {
            "command": " ".join(command),
            "status": "pass" if probe_passed else "fail",
            "exit_code": result.returncode,
            "log": str(args.log),
            "log_sha256": sha256(args.log),
        },
        "transitions": transition_rows(probe_passed),
        "privacy": {"fallback_key": False, "plaintext_queue": False, "silent_loss": False},
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "probe": "pass" if probe_passed else "fail"}))
    return 0 if probe_passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
