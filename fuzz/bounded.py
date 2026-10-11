#!/usr/bin/env python3
"""Run the existing synthetic native-boundary targets with bounded evidence."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import resource
import selectors
import shutil
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "frame_decoder", "schema_parser", "origin_validation", "pairing_state",
    "sequence_handling", "policy_conversion", "transport_differential",
)
NIGHTLY = "nightly-2026-10-03"
FUZZ_VERSION = "cargo-fuzz 0.13.2"


class CampaignInterrupted(KeyboardInterrupt):
    def __init__(self, number: int):
        self.number = number


def interrupt(number: int, _frame) -> None:
    raise CampaignInterrupted(number)


def limit(name: str, default: int, maximum: int) -> int:
    value = os.environ.get(name, str(default))
    if not value.isascii() or not value.isdecimal() or not 1 <= int(value) <= maximum:
        raise ValueError(f"{name} must be an integer from 1 through {maximum}")
    return int(value)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inputs() -> dict[str, str]:
    paths = [ROOT / "Cargo.lock", ROOT / "fuzz/Cargo.lock"]
    for directory in (ROOT / "fuzz/corpus", ROOT / "fuzz/regressions", ROOT / "fuzz/fuzz_targets"):
        if directory.exists():
            for path in sorted(directory.rglob("*")):
                if path.is_symlink():
                    raise ValueError("synthetic input directories must not contain symlinks")
                if path.is_file():
                    paths.append(path)
    return {str(path.relative_to(ROOT)): digest(path) for path in paths}


def tool(*command: str) -> str:
    return subprocess.run(command, cwd=ROOT, check=True, capture_output=True,
                          text=True, timeout=30).stdout.strip()


def kill_group(process: subprocess.Popen) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def execute(command: list[str], log: Path, deadline: int, log_limit: int) -> dict:
    """Drain a bounded log and kill the command's process group on a limit."""
    start = time.monotonic()
    cpu_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, start_new_session=True)
    reason = None
    interrupted_signal = None
    written = 0
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    try:
        with log.open("wb") as stream:
            while selector.get_map() or process.poll() is None:
                if reason is None and time.monotonic() - start > deadline:
                    reason = "wall_timeout"
                    kill_group(process)
                for key, _ in selector.select(0.1):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    stream.write(chunk[:max(0, log_limit - written)])
                    written += len(chunk)
                    if reason is None and written > log_limit:
                        reason = "log_limit"
                        kill_group(process)
        code = process.wait()
    except CampaignInterrupted as exc:
        reason = "interrupted"
        interrupted_signal = exc.number
        kill_group(process)
        code = process.wait()
    finally:
        if process.poll() is None:
            kill_group(process)
            process.wait()
        process.stdout.close()
        selector.close()
    cpu_after = resource.getrusage(resource.RUSAGE_CHILDREN)
    return {
        "command": command, "exit_code": code,
        "outcome": reason or ("passed" if code == 0 else "failed"),
        "interrupted_signal": interrupted_signal,
        "wall_seconds": round(time.monotonic() - start, 6),
        "cpu_seconds": round(cpu_after.ru_utime + cpu_after.ru_stime
                             - cpu_before.ru_utime - cpu_before.ru_stime, 6),
        "wall_limit_seconds": deadline, "log_limit_bytes": log_limit,
        "log_bytes": log.stat().st_size, "log_sha256": digest(log),
    }


def save(output: Path, receipt: dict) -> None:
    temporary = output / "receipt.json.tmp"
    temporary.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    temporary.replace(output / "receipt.json")


def finish(output: Path, receipt: dict, interrupted_signal: int | None,
           runner_error: str | None = None) -> int:
    before = receipt["inputs_before"]
    try:
        after = inputs()
    except (OSError, ValueError) as exc:
        validation_error = f"{type(exc).__name__}: {exc}"
        receipt["input_validation_error"] = validation_error
        receipt["inputs_after"] = None
        receipt["seeds_unchanged"] = None
        receipt["inputs_unchanged"] = None
        runner_error = runner_error or validation_error
    else:
        receipt["inputs_after"] = after
        receipt["seeds_unchanged"] = {k: v for k, v in before.items() if k.startswith("fuzz/corpus/")} == {
            k: v for k, v in after.items() if k.startswith("fuzz/corpus/")}
        receipt["inputs_unchanged"] = before == after
    passed = receipt["inputs_unchanged"] is True and len(receipt["targets"]) == len(TARGETS) and all(
        target["outcome"] == "passed" for target in receipt["targets"])
    receipt["outcome"] = ("interrupted" if interrupted_signal else "runner_error" if runner_error
                          else "passed" if passed else "failed")
    if runner_error:
        receipt["runner_error"] = runner_error
    receipt["interrupted_signal"] = interrupted_signal
    receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
    for target in receipt["targets"]:
        directory = output / target["target"]
        if directory.is_dir():
            save(directory, target)
    save(output, receipt)
    print(f"Receipt: {output / 'receipt.json'}", flush=True)
    if runner_error:
        print(f"bounded campaign: {runner_error}", file=sys.stderr)
    return 128 + interrupted_signal if interrupted_signal else 2 if runner_error else 0 if passed else 1


def main() -> int:
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupt)
    seconds = limit("FUZZ_SECONDS", 60, 3600)
    timeout = limit("FUZZ_TIMEOUT_SECS", 2, 60)
    rss = limit("FUZZ_RSS_MB", 256, 4096)
    max_len = limit("FUZZ_MAX_LEN", 65536, 65536)
    build_timeout = limit("FUZZ_BUILD_TIMEOUT_SECS", 900, 1800)
    log_limit = limit("FUZZ_MAX_LOG_BYTES", 4 * 1024 * 1024, 64 * 1024 * 1024)
    os.environ.setdefault("RUSTUP_TOOLCHAIN", NIGHTLY)
    os.environ.setdefault("ASAN_OPTIONS", "quarantine_size_mb=16")
    os.environ["CARGO_BUILD_JOBS"] = "1"
    os.environ["CARGO_NET_OFFLINE"] = "true"
    rustc = tool("rustc", "-vV")
    cargo_fuzz = tool("cargo", "fuzz", "--version")
    if os.environ["RUSTUP_TOOLCHAIN"] != NIGHTLY or cargo_fuzz != FUZZ_VERSION:
        raise ValueError(f"use {NIGHTLY} and {FUZZ_VERSION}")
    target_triple = os.environ.get("FUZZ_TARGET_TRIPLE") or next(
        line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: ")
    )
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = Path(os.environ.get("FUZZ_OUTPUT_DIR", f"fuzz/artifacts/campaign-{timestamp}-{os.getpid()}"))
    output = (ROOT / output).resolve()
    build_dir = (ROOT / os.environ.get("FUZZ_BUILD_DIR", "fuzz/target/bounded")).resolve()
    for protected in (ROOT / "fuzz/corpus", ROOT / "fuzz/regressions", ROOT / "fuzz/fuzz_targets"):
        if protected == output or protected in output.parents or protected == build_dir or protected in build_dir.parents:
            raise ValueError("output and build directories must be outside checked-in inputs")
    before = inputs()
    output.mkdir(parents=True, exist_ok=False)
    source = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    dirty = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True)
    receipt = {
        "schema_version": 1, "scope": "synthetic-native-boundaries",
        "source_revision": source.stdout.strip() if source.returncode == 0 else None,
        "source_tree_dirty": bool(dirty.stdout) if dirty.returncode == 0 else None,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "system": {"os": platform.system(), "release": platform.release(), "architecture": platform.machine()},
        "toolchain": {"rustup": os.environ["RUSTUP_TOOLCHAIN"], "rustc": rustc, "cargo_fuzz": cargo_fuzz},
        "target_triple": target_triple, "asan_options": os.environ["ASAN_OPTIONS"],
        "inputs_before": before, "targets": [], "outcome": "running",
    }
    options = ["--target", target_triple, "--target-dir", str(build_dir)]
    interrupted_signal = None
    result = None
    try:
        save(output, receipt)
        for target in TARGETS:
            result = {"target": target, "executions": None, "peak_rss_mb": None,
                      "replay_input_count": None, "artifacts": [], "outcome": "preparing"}
            receipt["targets"].append(result)
            directory = output / target
            directory.mkdir()
            crashes = directory / "crashes"
            crashes.mkdir()
            corpus = directory / "corpus"
            shutil.copytree(ROOT / "fuzz/corpus" / target, corpus)
            regression_dir = ROOT / "fuzz/regressions" / target
            if regression_dir.exists():
                for path in sorted(regression_dir.iterdir()):
                    if path.is_file():
                        shutil.copy2(path, corpus / f"regression-{digest(path)}")
            seed_paths = sorted(str(path) for path in corpus.iterdir() if path.is_file())
            result["replay_input_count"] = len(seed_paths)
            target_max_len = 131080 if target == "frame_decoder" else max_len
            flags = [f"-timeout={timeout}", f"-rss_limit_mb={rss}", f"-max_len={target_max_len}",
                     "-seed=1", "-print_final_stats=1", f"-artifact_prefix={crashes}/"]
            result["build"] = execute(["cargo", "fuzz", "build", target, *options],
                                      directory / "build.log", build_timeout, log_limit)
            result["outcome"] = "build_failed"
            if result["build"]["outcome"] == "passed":
                result["replay"] = execute(["cargo", "fuzz", "run", target, *seed_paths,
                                            *options, "--", "-runs=0", *flags],
                                           directory / "replay.log", 30 + timeout * len(seed_paths), log_limit)
                result["outcome"] = "replay_failed"
                if result["replay"]["outcome"] == "passed":
                    result["campaign"] = execute(["cargo", "fuzz", "run", target, str(corpus),
                                                  *options, "--", f"-max_total_time={seconds}", *flags],
                                                 directory / "campaign.log", seconds + timeout + 30, log_limit)
                    log = (directory / "campaign.log").read_text(errors="replace")
                    for field, stat in (("executions", "number_of_executed_units"), ("peak_rss_mb", "peak_rss_mb")):
                        match = re.search(rf"stat::{stat}:\s*(\d+)", log)
                        result[field] = int(match.group(1)) if match else None
                    result["outcome"] = result["campaign"]["outcome"]
                    if result["outcome"] == "failed":
                        lower = log.lower()
                        if "error: libfuzzer: timeout" in lower:
                            result["outcome"] = "timeout"
                        elif "error: libfuzzer: out-of-memory" in lower:
                            result["outcome"] = "out_of_memory"
                        elif ("error: libfuzzer: deadly signal" in lower or "panicked at" in lower
                              or "addresssanitizer:deadlysignal" in lower
                              or re.search(r"(?:error|summary): (?:address|leak|memory|thread|undefinedbehavior)sanitizer:", lower)):
                            result["outcome"] = "crash"
                        else:
                            result["outcome"] = "campaign_failed"
                    elif result["outcome"] == "passed" and (not result["executions"] or result["peak_rss_mb"] is None):
                        result["outcome"] = "missing_statistics"
                    elif result["outcome"] == "passed" and result["peak_rss_mb"] > rss:
                        result["outcome"] = "rss_limit_exceeded"
            for phase in ("build", "replay", "campaign"):
                if result.get(phase, {}).get("interrupted_signal"):
                    interrupted_signal = result[phase]["interrupted_signal"]
                    result["outcome"] = "interrupted"
            for path in sorted(crashes.iterdir()):
                if path.is_file():
                    result["artifacts"].append({"path": str(path.relative_to(output)),
                                                "bytes": path.stat().st_size, "sha256": digest(path)})
            save(directory, result)
            print(f"{target}: {result['outcome']}", flush=True)
            result = None
            save(output, receipt)
            if interrupted_signal is not None:
                break
        return finish(output, receipt, interrupted_signal)
    except CampaignInterrupted as exc:
        for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(number, signal.SIG_IGN)
        if result is not None:
            result["outcome"] = "interrupted"
        return finish(output, receipt, exc.number)
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        runner_error = f"{type(exc).__name__}: {exc}"
        if result is not None:
            result["outcome"] = "runner_error"
            result["runner_error"] = runner_error
        return finish(output, receipt, interrupted_signal, runner_error)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except CampaignInterrupted as exc:
        print("bounded campaign: interrupted before receipt creation", file=sys.stderr)
        sys.exit(128 + exc.number)
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"bounded campaign: {exc}", file=sys.stderr)
        sys.exit(2)
