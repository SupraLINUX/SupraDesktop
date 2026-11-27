#!/usr/bin/env bash
# Compare the existing lab lock against the workspace's provisional toolchain.
set -euo pipefail
out=$(realpath "$1")
cd "$RUNNER_TEMP/smithay-candidate"
cmp Cargo.lock "$out/upstream-Cargo.lock"
rustc +1.88.0 --version > "$out/compatibility-toolchain.txt"
cargo +1.88.0 --version >> "$out/compatibility-toolchain.txt"
rc=0
# Separate build directory prevents cross-toolchain contamination.
CARGO_TARGET_DIR="$RUNNER_TEMP/smithay-msrv-target" \
    cargo +1.88.0 check --locked --offline -p smallvil \
    > "$out/compatibility-check.log" 2>&1 || rc=$?
cmp Cargo.lock "$out/upstream-Cargo.lock"
cat "$out/compatibility-check.log"
printf '{"schema_version":1,"rust":"1.88.0","check_exit":%s,"same_lock":true,"runtime_tested":false}\n' "$rc" > "$out/compatibility-report.json"
cat "$out/compatibility-report.json"
if (( rc != 0 )); then
    echo "::warning::Rust 1.88 candidate check unsuccessful; inspect compatibility log before classifying"
fi
