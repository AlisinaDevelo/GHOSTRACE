#!/usr/bin/env bash
set -euo pipefail

# The lifecycle boundary is a macOS-only native FFI surface.  Run this lane on
# the host architecture with nightly's address sanitizer; do not silently fall
# back to a non-sanitized stable build.
if ! rustup run nightly rustc --version >/dev/null 2>&1; then
  echo "NO_GO: Rust nightly is unavailable; FSEvents sanitizer evidence is not claimed" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "NO_GO: FSEvents sanitizer requires a macOS host" >&2
  exit 2
fi

asan_runtime="$(rustc +nightly --print target-libdir)/librustc-nightly_rt.asan.dylib"
if [[ ! -f "$asan_runtime" ]]; then
  echo "NO_GO: nightly AddressSanitizer runtime is unavailable at $asan_runtime" >&2
  exit 2
fi

export ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=1:abort_on_error=1:halt_on_error=1}"

# Compile dependencies normally, then pass the unstable sanitizer flag only to
# the final integration-test crate.  Instrumenting proc-macro dylibs makes
# rustc load AddressSanitizer through dlopen and fails on macOS before the test
# starts; the two-phase cargo-rustc lane avoids that false failure while still
# instrumenting this adapter and its FFI calls.
cargo +nightly test --locked --test fsevents_lifecycle --no-run
cargo +nightly rustc --locked --test fsevents_lifecycle -- -Zsanitizer=address

target_dir="${CARGO_TARGET_DIR:-target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$PWD/$target_dir"
fi
sanitized_binary=""
while IFS= read -r candidate; do
  if otool -L "$candidate" 2>/dev/null | grep -q 'librustc-nightly_rt.asan.dylib'; then
    sanitized_binary="$candidate"
  fi
done < <(find "$target_dir/debug/build/ghostrace" -type f -name 'fsevents_lifecycle-*' -perm -111 -print 2>/dev/null)

if [[ -z "$sanitized_binary" ]]; then
  echo "NO_GO: cargo did not produce an AddressSanitizer integration-test binary" >&2
  exit 2
fi

# macOS loads the sanitizer runtime late unless it is inserted explicitly when
# the final test binary is launched.
DYLD_INSERT_LIBRARIES="$asan_runtime" "$sanitized_binary" --nocapture
