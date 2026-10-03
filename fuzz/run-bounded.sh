#!/bin/sh
set -eu

fuzz_seconds="${FUZZ_SECONDS:-60}"
timeout_seconds="${FUZZ_TIMEOUT_SECS:-2}"
rss_megabytes="${FUZZ_RSS_MB:-256}"
max_len="${FUZZ_MAX_LEN:-65536}"

targets='frame_decoder schema_parser origin_validation pairing_state sequence_handling policy_conversion transport_differential'

mkdir -p fuzz/artifacts
for target in $targets; do
    target_max_len="$max_len"
    if [ "$target" = frame_decoder ]; then
        target_max_len=131080
    fi
    log_path="fuzz/artifacts/${target}.log"
    cargo fuzz run "$target" -- \
        -max_total_time="$fuzz_seconds" \
        -timeout="$timeout_seconds" \
        -rss_limit_mb="$rss_megabytes" \
        -max_len="$target_max_len" \
        -print_final_stats=1 >"$log_path" 2>&1
    cat "$log_path"
done
