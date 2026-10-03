#!/usr/bin/env bash
# Build a universal (Apple silicon + Intel) ghostrace CLI from locked inputs
# and write a manifest that records every build input and output digest.
#
#   scripts/build-universal.sh [out-dir]          default: dist/universal
#   scripts/build-universal.sh --twice [out-dir]  build twice from two copies
#                                                 of the commit and compare
#
# Pinned: the Rust toolchain (rust-toolchain.toml), Cargo.lock (--locked),
# the minimum macOS version, the build timestamp (the commit time), the
# codegen units, and path prefixes (remapped so no build directory, home
# directory, or registry path is embedded). Recorded: the Xcode, SDK, linker,
# and lipo versions, which the script cannot pin. Default features only.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
MIN_MACOS=13.0
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

build_once() {
	local source=$1 out=$2
	local target_dir="$out/target"
	mkdir -p "$out"
	(
		cd "$source"
		export MACOSX_DEPLOYMENT_TARGET=$MIN_MACOS
		export SOURCE_DATE_EPOCH
		SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)
		export CARGO_INCREMENTAL=0
		export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
		export CARGO_PROFILE_RELEASE_DEBUG=0
		export CARGO_PROFILE_RELEASE_STRIP=symbols
		export CARGO_TARGET_DIR="$target_dir"
		# Remap both the given and the resolved spellings of each path (for
		# example /var/... and /private/var/...): the compiler records the
		# resolved one, and an unmapped path would embed the build directory,
		# and with it the user name, in panic locations.
		local resolved_source resolved_home
		resolved_source=$(cd "$source" && pwd -P)
		resolved_home=$(cd "$HOME" && pwd -P)
		export RUSTFLAGS="--remap-path-prefix=$resolved_source=/ghostrace --remap-path-prefix=$source=/ghostrace --remap-path-prefix=$resolved_home/.cargo=/cargo --remap-path-prefix=$resolved_home/.rustup=/rustup"
		export ZERO_AR_DATE=1
		for target in "${TARGETS[@]}"; do
			cargo build --release --locked --target "$target" --bin ghostrace >"$out/build-$target.log" 2>&1
		done
	)
	local slices=()
	for target in "${TARGETS[@]}"; do
		cp "$target_dir/$target/release/ghostrace" "$out/ghostrace-$target"
		slices+=("$out/ghostrace-$target")
	done
	lipo -create "${slices[@]}" -output "$out/ghostrace"
	# Ad-hoc signature: required to run on Apple silicon, and deterministic
	# because it covers only the code hash. Not Developer ID signing.
	codesign --force --sign - --identifier com.alisinadevelo.ghostrace "$out/ghostrace"
	rm -rf "$target_dir"
}

manifest() {
	local source=$1 out=$2
	python3 - "$source" "$out" "$MIN_MACOS" <<'PY'
import hashlib, json, subprocess, sys
source, out, min_macos = sys.argv[1:4]
def run(*args, cwd=None):
    return subprocess.run(args, capture_output=True, text=True, cwd=cwd).stdout.strip()
def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()
def uuid(path, arch):
    for line in run("dwarfdump", "--uuid", "--arch", arch, path).splitlines():
        return line.split()[1]
def minos(path, arch):
    lines = run("vtool", "-arch", arch, "-show-build", path).splitlines()
    return next((line.split()[1] for line in lines if line.strip().startswith("minos")), None)
universal = f"{out}/ghostrace"
slices = {"arm64": f"{out}/ghostrace-aarch64-apple-darwin", "x86_64": f"{out}/ghostrace-x86_64-apple-darwin"}
document = {
    "schema_version": 1,
    "source_revision": run("git", "rev-parse", "HEAD", cwd=source),
    "source_date_epoch": int(run("git", "log", "-1", "--format=%ct", cwd=source)),
    "cargo_lock_sha256": sha(f"{source}/Cargo.lock"),
    "rustc": run("rustc", "-vV", cwd=source).splitlines()[0],
    "cargo": run("cargo", "-V", cwd=source),
    "xcode": " ".join(run("xcodebuild", "-version").splitlines()),
    "sdk": run("xcrun", "--show-sdk-version"),
    "linker": subprocess.run(["ld", "-v"], capture_output=True, text=True).stderr.splitlines()[0],
    "minimum_macos": min_macos,
    "features": "default",
    "slices": {
        arch: {
            "sha256": sha(path),
            "uuid": uuid(universal, arch),
            "minos": minos(universal, arch),
            # Absolute build or home paths left in the binary; must be 0.
            "embedded_local_paths": sum(
                line.count("/Users/") + line.count("/private/var/folders") + line.count("/var/folders")
                for line in run("strings", "-a", path).splitlines()
            ),
        }
        for arch, path in slices.items()
    },
    "universal": {"sha256": sha(universal), "archs": run("lipo", "-archs", universal)},
}
json.dump(document, open(f"{out}/manifest.json", "w"), indent=2, sort_keys=True)
print(json.dumps(document["universal"]))
PY
}

if [ "${1:-}" = "--twice" ]; then
	OUT=${2:-$ROOT/dist/universal-twice}
	rm -rf "$OUT"
	mkdir -p "$OUT"
	revision=$(git -C "$ROOT" rev-parse HEAD)
	for run in a b; do
		# Each build uses its own clone at a different absolute path, so a
		# leaked build path would show up as a difference.
		clone=$(mktemp -d)/ghostrace-src-$run
		git -c init.templateDir= clone -q --no-hardlinks "$ROOT" "$clone"
		git -C "$clone" checkout -q "$revision"
		build_once "$clone" "$OUT/$run"
		manifest "$clone" "$OUT/$run" >/dev/null
		rm -rf "$(dirname "$clone")"
	done
	python3 - "$OUT" <<'PY'
import json, sys
out = sys.argv[1]
a, b = (json.load(open(f"{out}/{run}/manifest.json")) for run in "ab")
differences = []
def compare(prefix, left, right):
    if isinstance(left, dict):
        for key in sorted(set(left) | set(right)):
            compare(f"{prefix}.{key}" if prefix else key, left.get(key), right.get(key))
    elif left != right:
        differences.append({"field": prefix, "a": left, "b": right})
compare("", a, b)
report = {"schema_version": 1, "revision": a["source_revision"], "identical": not differences, "differences": differences}
json.dump(report, open(f"{out}/comparison.json", "w"), indent=2)
print(json.dumps(report, indent=2))
sys.exit(0 if not differences else 1)
PY
else
	OUT=${1:-$ROOT/dist/universal}
	rm -rf "$OUT"
	build_once "$ROOT" "$OUT"
	manifest "$ROOT" "$OUT"
fi
