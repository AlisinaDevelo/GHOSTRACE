#!/usr/bin/env bash
set -euo pipefail

# GHOSTRACE-0048: clean-machine smoke for the pinned fixture-only path.
# Dependency installation is explicit and documented separately; all checks
# below run with the already installed 1.88.0 toolchain and no network access.

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

export CARGO_NET_OFFLINE=true
WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/ghostrace-repro.XXXXXX")
trap 'rm -rf "$WORK_DIR"' EXIT

echo "reproducibility: pinned inputs"
python3 scripts/reproducibility.py check
python3 scripts/fixture-manifest.py check
python3 scripts/identity-audit.py check

echo "reproducibility: rustfmt"
cargo +1.88.0 fmt --all -- --check

echo "reproducibility: schema"
cargo +1.88.0 run --quiet -- schema > "$WORK_DIR/schema.json"
python3 - "$WORK_DIR/schema.json" schemas/event-envelope-v1.json <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as generated, open(sys.argv[2], encoding="utf-8") as checked_in:
    if json.load(generated) != json.load(checked_in):
        raise SystemExit("schema output differs from the checked-in contract")
PY

echo "reproducibility: deterministic demo"
event_id=00000000-0000-4000-8000-000000000008
cargo +1.88.0 run --quiet -- demo --fixture fixtures/causal-chain.jsonl --event "$event_id" > "$WORK_DIR/demo-a.json"
cargo +1.88.0 run --quiet -- demo --fixture fixtures/causal-chain.jsonl --event "$event_id" > "$WORK_DIR/demo-b.json"
cmp -s "$WORK_DIR/demo-a.json" "$WORK_DIR/demo-b.json"

echo "reproducibility: deterministic export"
cargo +1.88.0 run --quiet -- export --fixture fixtures/causal-chain.jsonl --output "$WORK_DIR/export-a.jsonl"
cargo +1.88.0 run --quiet -- export --fixture fixtures/causal-chain.jsonl --output "$WORK_DIR/export-b.jsonl"
cmp -s "$WORK_DIR/export-a.jsonl" "$WORK_DIR/export-b.jsonl"

echo "reproducibility: capture refusal"
if cargo +1.88.0 run --quiet -- capture > "$WORK_DIR/capture.stdout" 2> "$WORK_DIR/capture.stderr"; then
  echo "capture unexpectedly succeeded" >&2
  exit 1
fi
grep -F "live capture is intentionally disabled" "$WORK_DIR/capture.stderr" >/dev/null

echo "reproducibility: roadmap and Python evidence"
python3 scripts/roadmap.py check >/dev/null
python3 -m unittest discover -s tests -p 'test_*.py'

echo "reproducibility: Rust evidence"
cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.88.0 test --locked --all-targets --all-features

echo "reproducibility: all checks passed"
