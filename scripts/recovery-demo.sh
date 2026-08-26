#!/usr/bin/env bash
set -euo pipefail

# Device-side MVP: create two synthetic unreferenced events, sign a checkpoint,
# repair one interval on a verified copy, and validate the path-free manifest.
ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

manifest=$(cargo +1.88.0 run --quiet --locked -- recovery-demo)
python3 -c '
import json
import sys

manifest = json.loads(sys.argv[1])
if not manifest["verified_copy"]:
    raise SystemExit("repair did not operate on a verified copy")
if manifest["dropped_event_count"] != 1 or manifest["gap_event_count"] != 1:
    raise SystemExit("MVP repair counts drifted")
if manifest["after"]["event_count"] != manifest["before"]["event_count"]:
    raise SystemExit("gap replacement did not reconcile event count")
if "/private/tmp" in sys.argv[1] or "recovery-demo-v1" in sys.argv[1]:
    raise SystemExit("MVP manifest leaked a path or key seed")
' "$manifest"
printf '%s\n' "$manifest"
