#!/usr/bin/env bash
set -euo pipefail

# GHOSTRACE-0048: clean-machine smoke for the pinned fixture-only path.
# Dependency installation is explicit and documented separately; all checks
# below run with the already installed 1.88.0 toolchain and no network access.

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export CARGO_INCREMENTAL=0
WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/ghostrace-repro.XXXXXX")
trap 'rm -rf "$WORK_DIR"' EXIT

echo "reproducibility: pinned inputs"
python3 scripts/reproducibility.py check
python3 scripts/fixture-manifest.py check
python3 scripts/fsevents-lifecycle-corpus.py check
python3 scripts/filesystem-benchmark.py check
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

echo "reproducibility: durable fixture CLI"
journal="$WORK_DIR/journal.sqlite3"
cargo +1.88.0 run --quiet -- init --journal "$journal"
cargo +1.88.0 run --quiet -- init --journal "$journal"
cargo +1.88.0 run --quiet -- ingest --journal "$journal" --fixture fixtures/causal-chain.jsonl
cargo +1.88.0 run --quiet -- explain --journal "$journal" --event "$event_id" > "$WORK_DIR/explain-a.json"
cargo +1.88.0 run --quiet -- explain --journal "$journal" --event "$event_id" > "$WORK_DIR/explain-b.json"
cmp -s "$WORK_DIR/explain-a.json" "$WORK_DIR/explain-b.json"
python3 - "$WORK_DIR/explain-a.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    explanation = json.load(handle)
if len(explanation["chain_event_ids"]) != 8 or explanation["coverage"]["gap_event_count"] != 1:
    raise SystemExit("durable explanation coverage drifted")
PY

echo "reproducibility: deterministic export"
export_confirmed() {
  local input_flag=$1
  local input_path=$2
  local output_path=$3
  local preview_path=$4
  cargo +1.88.0 run --quiet -- preview "$input_flag" "$input_path" --output "$output_path" > "$preview_path"
  local plan_digest
  local snapshot_digest
  plan_digest=$(python3 - "$preview_path" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    print(json.load(handle)["plan_digest"])
PY
  )
  snapshot_digest=$(python3 - "$preview_path" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    print(json.load(handle)["snapshot_digest"])
PY
  )
  cargo +1.88.0 run --quiet -- export "$input_flag" "$input_path" --output "$output_path" \
    --confirm-plan "$plan_digest" --confirm-snapshot "$snapshot_digest"
}

export_confirmed --fixture fixtures/causal-chain.jsonl "$WORK_DIR/export-a.jsonl" "$WORK_DIR/export-a.preview.json"
export_confirmed --fixture fixtures/causal-chain.jsonl "$WORK_DIR/export-b.jsonl" "$WORK_DIR/export-b.preview.json"
cmp -s "$WORK_DIR/export-a.jsonl" "$WORK_DIR/export-b.jsonl"
export_confirmed --journal "$journal" "$WORK_DIR/export-journal.jsonl" "$WORK_DIR/export-journal.preview.json"
cargo +1.88.0 run --quiet -- validate --export "$WORK_DIR/export-journal.jsonl" | grep -F "validated 8 event(s)" >/dev/null
python3 - "$WORK_DIR/export-journal.jsonl" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    records = [json.loads(line) for line in handle if line.strip()]
if records[0]["record_type"] != "manifest" or records[0]["coverage"]["event_count"] != 8:
    raise SystemExit("durable export manifest drifted")
if len(records) != 9:
    raise SystemExit("durable export record count drifted")
PY

echo "reproducibility: retention dry-run"
cargo +1.88.0 run --quiet -- retention-plan \
  --journal "$journal" \
  --before 2026-01-01T00:00:08Z \
  > "$WORK_DIR/retention-a.json"
cargo +1.88.0 run --quiet -- retention-plan \
  --journal "$journal" \
  --before 2026-01-01T00:00:08Z \
  > "$WORK_DIR/retention-b.json"
cmp -s "$WORK_DIR/retention-a.json" "$WORK_DIR/retention-b.json"
python3 - "$WORK_DIR/retention-a.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    plan = json.load(handle)
if plan["snapshot_event_count"] != 8 or plan["affected_event_count"] != 7:
    raise SystemExit("retention dry-run scope drifted")
if plan["protected_gap_count"] != 1 or not plan["candidate_set_digest"].startswith("sha256:"):
    raise SystemExit("retention coverage binding drifted")
PY

echo "reproducibility: retention residue report"
cargo +1.88.0 run --quiet -- residue-report \
  --journal "$journal" \
  > "$WORK_DIR/residue-a.json"
cargo +1.88.0 run --quiet -- residue-report \
  --journal "$journal" \
  > "$WORK_DIR/residue-b.json"
cmp -s "$WORK_DIR/residue-a.json" "$WORK_DIR/residue-b.json"
python3 - "$WORK_DIR/residue-a.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
if report["schema_version"] != 1 or len(report["modes"]) != 4:
    raise SystemExit("retention residue mode contract drifted")
if report["external_backup_count"] != 0 or len(report["artifacts"]) != 8:
    raise SystemExit("retention residue inventory drifted")
if any("journal.sqlite3" in value for value in report.get("notes", [])):
    raise SystemExit("retention residue report leaked a path")
PY

echo "reproducibility: transactional retention confirmation"
cargo +1.88.0 run --quiet -- retention-plan \
  --journal "$journal" \
  --before 1970-01-01T00:00:00Z \
  > "$WORK_DIR/empty-retention-plan.json"
read -r retention_plan_digest retention_candidate_digest retention_boundary < <(
  python3 - "$WORK_DIR/empty-retention-plan.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    plan = json.load(handle)
print(plan["plan_digest"], plan["candidate_set_digest"], plan["snapshot_boundary"])
PY
)
cargo +1.88.0 run --quiet -- retention-delete \
  --journal "$journal" \
  --before 1970-01-01T00:00:00Z \
  --confirm-plan "$retention_plan_digest" \
  --confirm-candidate-set "$retention_candidate_digest" \
  --confirm-snapshot-boundary "$retention_boundary" \
  > "$WORK_DIR/empty-retention-receipt.json"
python3 - "$WORK_DIR/empty-retention-receipt.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    receipt = json.load(handle)
if receipt["requested_event_count"] != 0 or receipt["deleted_event_count"] != 0:
    raise SystemExit("empty retention confirmation deleted an unexpected row")
if receipt["compaction_performed"] or not receipt["external_copies_untouched"]:
    raise SystemExit("retention deletion receipt boundary drifted")
PY

echo "reproducibility: integrity check"
cargo +1.88.0 run --quiet -- integrity-check --journal "$journal" \
  > "$WORK_DIR/integrity-a.json"
cargo +1.88.0 run --quiet -- integrity-check --journal "$journal" \
  > "$WORK_DIR/integrity-b.json"
cmp -s "$WORK_DIR/integrity-a.json" "$WORK_DIR/integrity-b.json"
python3 - "$WORK_DIR/integrity-a.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
if report["schema_version"] != 1 or not report["integrity_ok"]:
    raise SystemExit("integrity check did not pass")
if len(report["recovery_guidance"]) != 4:
    raise SystemExit("integrity recovery guidance drifted")
PY

echo "reproducibility: authenticated journal state"
cargo +1.88.0 run --quiet -- authenticated-check --journal "$journal" \
  > "$WORK_DIR/authenticated-a.json"
cargo +1.88.0 run --quiet -- authenticated-check --journal "$journal" \
  > "$WORK_DIR/authenticated-b.json"
cmp -s "$WORK_DIR/authenticated-a.json" "$WORK_DIR/authenticated-b.json"
python3 - "$WORK_DIR/authenticated-a.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
if not report["valid"] or not report["local_key_only"]:
    raise SystemExit("authenticated journal state did not verify")
if report["event_count"] != 8 or report["stored_event_count"] != 8:
    raise SystemExit("authenticated event count drifted")
if report["anomalies"]:
    raise SystemExit("authenticated report unexpectedly contains anomalies")
PY

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
