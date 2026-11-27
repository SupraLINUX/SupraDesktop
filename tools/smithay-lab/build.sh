#!/usr/bin/env bash
# External source in runner temp only. No import into SupraDesktop's workspace.
set -euo pipefail
out=$(realpath -m "$1")
mkdir -p "$out"
upstream="$RUNNER_TEMP/smithay-candidate"
commit=a166cf4c94b5aedc332a65aa1dd753e8148829c3
git init -q "$upstream"
git -C "$upstream" remote add origin https://github.com/Smithay/smithay.git
git -C "$upstream" fetch --depth=1 origin "$commit"
git -C "$upstream" checkout --detach FETCH_HEAD
test "$(git -C "$upstream" rev-parse HEAD)" = "$commit"
printf '%s\n' "$commit" > "$out/upstream-commit.txt"
sha256sum "$upstream/LICENSE.txt" > "$out/upstream-license.sha256"
cp "$upstream/LICENSE.txt" "$out/upstream-LICENSE.txt"
# Upstream does not track Cargo.lock at this revision: record the resolution,
# but do not claim reproducibility across clean runs.
cd "$upstream"
rustc +1.100.0 --version > "$out/toolchain.txt"
cargo +1.100.0 --version >> "$out/toolchain.txt"
cargo +1.100.0 generate-lockfile
cp Cargo.lock "$out/upstream-Cargo.lock"
cargo +1.100.0 tree --locked -p smallvil --target x86_64-unknown-linux-gnu \
    -e normal,build --prefix none --format '{p}|{l}' > "$out/dependency-licenses.txt"
cargo +1.100.0 tree --locked -p smallvil --target x86_64-unknown-linux-gnu \
    -e features > "$out/dependency-features.txt"
cargo +1.100.0 build --locked -p smallvil 2>&1 | tee "$out/build.log"
test -x target/debug/smallvil
