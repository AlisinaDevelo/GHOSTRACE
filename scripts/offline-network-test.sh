#!/usr/bin/env bash
set -euo pipefail

# GHOSTRACE-0044: run the fixture path under an explicit network-denial
# mechanism. Dependency download is deliberately outside this script's child
# process; the child receives CARGO_NET_OFFLINE=true and must not fetch.

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
Usage:
  scripts/offline-network-test.sh              Run with the native local denial mechanism.
  scripts/offline-network-test.sh --inside     Run after a caller has installed the denial.

The hosted workflow installs Docker --network=none before invoking --inside.
On macOS, the default mode installs sandbox-exec's deny network* profile.
EOF
}

run_inside() {
  : "${GHOSTRACE_OFFLINE_ENFORCED:?GHOSTRACE_OFFLINE_ENFORCED must be 1}"
  [[ "$GHOSTRACE_OFFLINE_ENFORCED" == "1" ]] || {
    echo "offline network canary was not enabled" >&2
    return 1
  }
  : "${GHOSTRACE_OFFLINE_MODE:?GHOSTRACE_OFFLINE_MODE is required}"

  local cargo_command=(cargo)
  # The hosted image already contains the pinned toolchain. Bypass rustup's
  # proxy there: the proxy may try to refresh channel metadata even when the
  # requested toolchain is installed, which would turn a correct denial into a
  # misleading toolchain-download failure.
  local image_toolchain_bin
  local image_toolchain_found=0
  for image_toolchain_bin in /usr/local/rustup/toolchains/1.88.0-*/bin; do
    if [[ -x "$image_toolchain_bin/cargo" ]]; then
      PATH="$image_toolchain_bin:$PATH"
      export PATH
      cargo_command=(cargo)
      image_toolchain_found=1
      break
    fi
  done
  if [[ "$image_toolchain_found" == "0" ]]; then
    cargo_command=(cargo +1.88.0)
  fi

  echo "offline-lane mode=$GHOSTRACE_OFFLINE_MODE"
  "${cargo_command[@]}" --version
  rustc --version

  echo "offline-lane canary"
  CARGO_NET_OFFLINE=true "${cargo_command[@]}" test --locked --offline --test offline_network_canary -- --ignored --nocapture

  echo "offline-lane privacy fixture/explanation/export"
  CARGO_NET_OFFLINE=true "${cargo_command[@]}" test --locked --offline --test privacy_regression -- --test-threads=1

  echo "offline-lane complete product suite"
  CARGO_NET_OFFLINE=true "${cargo_command[@]}" test --locked --offline --all-targets --all-features
}

case "${1:-}" in
  --inside)
    [[ "$#" == 1 ]] || { usage >&2; exit 2; }
    run_inside
    ;;
  "")
    case "$(uname -s)" in
      Darwin)
        command -v /usr/bin/sandbox-exec >/dev/null 2>&1 || {
          echo "sandbox-exec is unavailable; run the hosted Docker lane instead" >&2
          exit 2
        }
        exec env \
          GHOSTRACE_OFFLINE_ENFORCED=1 \
          GHOSTRACE_OFFLINE_MODE=sandbox-exec \
          /usr/bin/sandbox-exec \
          -p '(version 1) (allow default) (deny network*)' \
          "$0" --inside
        ;;
      *)
        cat >&2 <<'EOF'
This host is not macOS, so no native runner was selected. Use the checked-in
workflow's Docker --network=none invocation, or explicitly invoke --inside from
an isolated network namespace with GHOSTRACE_OFFLINE_MODE set.
EOF
        exit 2
        ;;
    esac
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
