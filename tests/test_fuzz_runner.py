"""Exercise bounded runner file handling without compiling or fuzzing Rust."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "frame_decoder", "schema_parser", "origin_validation", "pairing_state",
    "sequence_handling", "policy_conversion", "transport_differential",
)

# Cargo is an external build/fuzz boundary. Synthetic output lets these tests
# check the real runner's files, limits, and failure retention in seconds.
FAKE_CARGO = r'''
import json, os, sys
from pathlib import Path
args = sys.argv[1:]
root = Path(os.environ["FUZZ_TEST_ROOT"])
with (root / "calls.jsonl").open("a") as stream:
    stream.write(json.dumps(args) + "\n")
if args == ["fuzz", "--version"]:
    print("cargo-fuzz 0.13.2")
    sys.exit(0)
target = args[2]
mode = os.environ.get("FUZZ_TEST_MODE", "")
if args[1] == "build":
    sys.exit(37 if mode == "build-failure" and target == "pairing_state" else 0)
def flag(prefix):
    return next(arg[len(prefix):] for arg in args if arg.startswith(prefix))
if "-runs=0" in args:
    print("fixed synthetic replay complete")
    sys.exit(0)
failures = {
    "tool-failure": "error: could not execute process: Exec format error",
    "timeout": "ERROR: libFuzzer: timeout after 2 seconds",
    "out-of-memory": "ERROR: libFuzzer: out-of-memory",
    "sanitizer": "ERROR: AddressSanitizer: heap-buffer-overflow",
    "panic": "thread '<unnamed>' panicked at fuzz_targets/frame_decoder.rs:42: assertion failed",
}
if target == "frame_decoder" and mode in failures:
    print(failures[mode])
    sys.exit(1)
if mode == "interrupt" and target == "frame_decoder":
    print("synthetic campaign interrupted", flush=True)
    os.kill(os.getppid(), 2)
    sys.exit(0)
corpus = Path(args[3])
(corpus / "generated").write_bytes(b"synthetic mutation")
if mode == "seed-change" and target == "frame_decoder":
    (root / "fuzz/corpus/frame_decoder/seed").write_bytes(b"changed seed")
if mode == "missing-lock" and target == "frame_decoder":
    (root / "Cargo.lock").unlink()
if mode == "crash" and target == "schema_parser":
    crash = Path(flag("-artifact_prefix=")) / "crash-synthetic"
    crash.write_bytes(b"synthetic failing input")
    print("Running target with -timeout=2")
    print("ERROR: libFuzzer: deadly signal")
    sys.exit(1)
if mode == "log-limit" and target == "frame_decoder":
    print("x" * 8192)
else:
    print("stat::number_of_executed_units: 128")
    print("stat::peak_rss_mb: 42")
'''

INTERRUPT_BETWEEN_PHASES = r'''
import os, runpy, shutil, signal
from pathlib import Path
boundary = os.environ["FUZZ_TEST_BOUNDARY"]
number = int(os.environ["FUZZ_TEST_SIGNAL"])
copytree = shutil.copytree
read_text = Path.read_text
def copy_then_interrupt(source, destination, *args, **kwargs):
    result = copytree(source, destination, *args, **kwargs)
    if boundary == "corpus" and Path(source).name == "schema_parser":
        os.kill(os.getpid(), number)
    return result
def read_then_interrupt(path, *args, **kwargs):
    result = read_text(path, *args, **kwargs)
    if boundary == "statistics" and path.name == "campaign.log" and path.parent.name == "schema_parser":
        os.kill(os.getpid(), number)
    return result
shutil.copytree = copy_then_interrupt
Path.read_text = read_then_interrupt
runpy.run_path("fuzz/bounded.py", run_name="__main__")
'''


class BoundedFuzzRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        fuzz_dir = self.root / "fuzz"
        fuzz_dir.mkdir()
        for name in ("run-bounded.sh", "bounded.py"):
            source = ROOT / "fuzz" / name
            if source.exists():
                shutil.copy2(source, fuzz_dir / name)
        for target in TARGETS:
            corpus = fuzz_dir / "corpus" / target
            corpus.mkdir(parents=True)
            (corpus / "seed").write_bytes(b"synthetic seed")
        (fuzz_dir / "Cargo.lock").write_text("synthetic lock")
        (self.root / "Cargo.lock").write_text("synthetic production lock")
        tools = self.root / "bin"
        tools.mkdir()
        cargo = tools / "cargo"
        cargo.write_text(f"#!{sys.executable}\n{FAKE_CARGO}")
        rustc = tools / "rustc"
        rustc.write_text(
            "#!/bin/sh\nprintf 'rustc 1.101.0-nightly (synthetic)\\nhost: x86_64-unknown-linux-gnu\\n'\n"
        )
        for tool in (cargo, rustc):
            tool.chmod(0o700)
        self.output = fuzz_dir / "artifacts" / "test-campaign"
        self.env = {
            **os.environ,
            "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
            "FUZZ_TEST_ROOT": str(self.root),
            "FUZZ_OUTPUT_DIR": str(self.output),
            "FUZZ_BUILD_DIR": str(fuzz_dir / "target"),
            "FUZZ_SECONDS": "1",
            "FUZZ_MAX_LOG_BYTES": "1024",
        }

    def run_campaign(self, mode: str = "") -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["sh", "fuzz/run-bounded.sh"], cwd=self.root,
            env={**self.env, "FUZZ_TEST_MODE": mode}, capture_output=True,
            text=True, timeout=20, check=False,
        )

    def receipt(self) -> dict:
        path = self.output / "receipt.json"
        self.assertTrue(path.is_file(), "runner did not retain its campaign receipt")
        return json.loads(path.read_text())

    def test_all_targets_replay_then_fuzz_without_changing_checked_in_seeds(self) -> None:
        result = self.run_campaign()
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = self.receipt()
        self.assertTrue(receipt["seeds_unchanged"])
        self.assertEqual([target["target"] for target in receipt["targets"]], list(TARGETS))
        for target in receipt["targets"]:
            self.assertEqual(target["outcome"], "passed")
            self.assertEqual(target["executions"], 128)
            self.assertEqual(target["peak_rss_mb"], 42)
            self.assertEqual(target["replay"]["outcome"], "passed")
            seed = self.root / "fuzz/corpus" / target["target"] / "seed"
            self.assertEqual(seed.read_bytes(), b"synthetic seed")
            log = self.output / target["target"] / "campaign.log"
            self.assertEqual(target["campaign"]["log_sha256"], hashlib.sha256(log.read_bytes()).hexdigest())
        calls = [json.loads(line) for line in (self.root / "calls.jsonl").read_text().splitlines()]
        replays = [call for call in calls if "-runs=0" in call]
        self.assertEqual(len(replays), 7)
        self.assertTrue(all("-seed=1" in call for call in replays))
        self.assertTrue(all("--target-dir" in call for call in replays))

    def test_target_failure_retains_crash_and_logs_and_runs_remaining_targets(self) -> None:
        result = self.run_campaign("crash")
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt()
        self.assertEqual(len(receipt["targets"]), 7)
        failed = receipt["targets"][1]
        self.assertEqual(failed["outcome"], "crash")
        self.assertEqual(failed["artifacts"][0]["sha256"], hashlib.sha256(b"synthetic failing input").hexdigest())
        self.assertTrue((self.output / "schema_parser/crashes/crash-synthetic").is_file())
        self.assertEqual(receipt["targets"][-1]["outcome"], "passed")

    def test_campaign_failure_requires_evidence_before_reporting_a_crash(self) -> None:
        for mode, outcome in (("tool-failure", "campaign_failed"), ("timeout", "timeout"),
                              ("out-of-memory", "out_of_memory"), ("sanitizer", "crash"),
                              ("panic", "crash")):
            with self.subTest(mode=mode):
                self.output = self.root / "fuzz/artifacts" / mode
                self.env["FUZZ_OUTPUT_DIR"] = str(self.output)
                result = self.run_campaign(mode)
                self.assertEqual(result.returncode, 1, result.stderr)
                receipt = self.receipt()
                self.assertEqual(receipt["outcome"], "failed")
                self.assertEqual(receipt["targets"][0]["outcome"], outcome)
                self.assertEqual(len(receipt["targets"]), 7)

    def test_interrupt_retains_current_phase_receipt_and_stops_campaign(self) -> None:
        result = self.run_campaign("interrupt")
        self.assertEqual(result.returncode, 130)
        receipt = self.receipt()
        self.assertEqual(receipt["outcome"], "interrupted")
        self.assertEqual(len(receipt["targets"]), 1)
        target = receipt["targets"][0]
        self.assertEqual(target["outcome"], "interrupted")
        self.assertEqual(target["campaign"]["outcome"], "interrupted")
        log = self.output / "frame_decoder/campaign.log"
        self.assertEqual(target["campaign"]["log_sha256"], hashlib.sha256(log.read_bytes()).hexdigest())

    def test_build_failure_retains_receipt_without_claiming_a_campaign_ran(self) -> None:
        result = self.run_campaign("build-failure")
        self.assertNotEqual(result.returncode, 0)
        failed = self.receipt()["targets"][3]
        self.assertEqual(failed["outcome"], "build_failed")
        self.assertNotIn("campaign", failed)
        self.assertIsNone(failed["executions"])
        self.assertTrue((self.output / "pairing_state/build.log").is_file())

    def test_between_phase_signals_finalize_receipt_and_retain_completed_work(self) -> None:
        harness = self.root / "interrupt-between-phases.py"
        harness.write_text(INTERRUPT_BETWEEN_PHASES)
        for boundary in ("corpus", "statistics"):
            for number in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
                with self.subTest(boundary=boundary, signal=number):
                    self.output = self.root / "fuzz/artifacts" / f"{boundary}-{number}"
                    result = subprocess.run(
                        [sys.executable, str(harness)], cwd=self.root,
                        env={**self.env, "FUZZ_OUTPUT_DIR": str(self.output),
                             "FUZZ_TEST_BOUNDARY": boundary, "FUZZ_TEST_SIGNAL": str(number)},
                        capture_output=True, text=True, timeout=20, check=False,
                    )
                    self.assertEqual(result.returncode, 128 + number, result.stderr)
                    receipt = self.receipt()
                    self.assertEqual(receipt["outcome"], "interrupted")
                    self.assertEqual(receipt["interrupted_signal"], number)
                    self.assertIn("finished_at", receipt)
                    self.assertTrue(receipt["inputs_unchanged"])
                    self.assertNotIn("before execution", result.stderr)
                    self.assertEqual([entry["target"] for entry in receipt["targets"]],
                                     ["frame_decoder", "schema_parser"])
                    self.assertEqual(receipt["targets"][0]["outcome"], "passed")
                    current = receipt["targets"][1]
                    self.assertEqual(current["outcome"], "interrupted")
                    target_receipt = self.output / "schema_parser/receipt.json"
                    self.assertEqual(json.loads(target_receipt.read_text()), current)
                    if boundary == "corpus":
                        self.assertNotIn("build", current)
                        self.assertNotIn("campaign", current)
                    else:
                        for phase in ("build", "replay", "campaign"):
                            self.assertEqual(current[phase]["outcome"], "passed")
                            log = self.output / "schema_parser" / f"{phase}.log"
                            self.assertEqual(current[phase]["log_sha256"],
                                             hashlib.sha256(log.read_bytes()).hexdigest())

    def test_missing_seed_directory_retains_a_terminal_runner_error(self) -> None:
        shutil.rmtree(self.root / "fuzz/corpus/schema_parser")
        result = self.run_campaign()
        self.assertEqual(result.returncode, 2, result.stderr)
        receipt = self.receipt()
        self.assertEqual(receipt["outcome"], "runner_error")
        self.assertIn("FileNotFoundError", receipt["runner_error"])
        self.assertIn("finished_at", receipt)
        self.assertTrue(receipt["inputs_unchanged"])
        self.assertEqual([target["target"] for target in receipt["targets"]],
                         ["frame_decoder", "schema_parser"])
        self.assertEqual(receipt["targets"][0]["outcome"], "passed")
        current = receipt["targets"][1]
        self.assertEqual(current["outcome"], "runner_error")
        self.assertNotIn("build", current)
        self.assertNotIn("campaign", current)
        self.assertEqual(json.loads((self.output / "schema_parser/receipt.json").read_text()), current)

    def test_failed_final_input_validation_is_retained_without_claiming_integrity(self) -> None:
        result = self.run_campaign("missing-lock")
        self.assertEqual(result.returncode, 2, result.stderr)
        receipt = self.receipt()
        self.assertEqual(receipt["outcome"], "runner_error")
        self.assertIn("Cargo.lock", receipt["input_validation_error"])
        self.assertIsNone(receipt["inputs_after"])
        self.assertIsNone(receipt["inputs_unchanged"])
        self.assertIsNone(receipt["seeds_unchanged"])
        self.assertIn("finished_at", receipt)
        self.assertEqual(len(receipt["targets"]), 7)
        self.assertTrue(all(target["outcome"] == "passed" for target in receipt["targets"]))

    def test_log_limit_retains_bounded_log_and_reports_failure(self) -> None:
        result = self.run_campaign("log-limit")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.receipt()["targets"][0]["outcome"], "log_limit")
        self.assertLessEqual((self.output / "frame_decoder/campaign.log").stat().st_size, 1024)

    def test_seed_drift_cannot_be_reported_as_success(self) -> None:
        result = self.run_campaign("seed-change")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.receipt()["seeds_unchanged"])

    def test_existing_campaign_is_preserved(self) -> None:
        self.output.mkdir(parents=True)
        sentinel = self.output / "retained-evidence"
        sentinel.write_text("keep")
        result = self.run_campaign()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(sentinel.read_text(), "keep")
        self.assertEqual(list(self.output.iterdir()), [sentinel])

    def test_output_cannot_be_created_inside_checked_in_seeds(self) -> None:
        protected = self.root / "fuzz/corpus/new-campaign"
        self.env["FUZZ_OUTPUT_DIR"] = str(protected)
        result = self.run_campaign()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(protected.exists())

    def test_invalid_limits_are_rejected_before_creating_output(self) -> None:
        for value in ("0", "-1", "unbounded"):
            with self.subTest(value=value):
                self.env["FUZZ_SECONDS"] = value
                result = self.run_campaign()
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.output.exists())

    def test_closed_output_still_respects_wall_deadline(self) -> None:
        spec = importlib.util.spec_from_file_location("bounded", ROOT / "fuzz/bounded.py")
        bounded = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(bounded)

        def exceeded_deadline(*_args) -> None:
            self.fail("runner waited after its wall deadline")

        previous = signal.signal(signal.SIGALRM, exceeded_deadline)
        signal.alarm(4)
        try:
            result = bounded.execute(
                [sys.executable, "-c", "import os,time; os.close(1); os.close(2); time.sleep(60)"],
                self.root / "closed-output.log", 1, 1024,
            )
        finally:
            signal.alarm(0)
            signal.signal(signal.SIGALRM, previous)
        self.assertEqual(result["outcome"], "wall_timeout")
        self.assertLess(result["wall_seconds"], 3)


if __name__ == "__main__":
    unittest.main()
