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
# External lab resolution must match the reviewed lock, not silently drift.
# The exact lock bytes are currently evidence, not a vendored upstream lock.
cd "$upstream"
rustc +1.100.0 --version > "$out/lock-generator-toolchain.txt"
cargo +1.100.0 --version >> "$out/lock-generator-toolchain.txt"
cargo +1.100.0 generate-lockfile
cp Cargo.lock "$out/upstream-Cargo.lock"
sha256sum Cargo.lock > "$out/upstream-lock.sha256"
expected=500aa0dde8b3d01ad7a106ab428db84c90d0ec4c1f04c60b225e6624a4278577
if ! printf '%s  Cargo.lock\n' "$expected" | sha256sum --check --strict; then
    echo 'BLOCKED: upstream dependency resolution drifted from reviewed evidence' >&2
    exit 1
fi
rustc +1.88.0 --version > "$out/toolchain.txt"
cargo +1.88.0 --version >> "$out/toolchain.txt"
cargo +1.88.0 tree --locked -p smallvil --target x86_64-unknown-linux-gnu \
    -e normal,build --prefix none --format '{p}|{l}' > "$out/dependency-licenses.txt"
cargo +1.88.0 tree --locked -p smallvil --target x86_64-unknown-linux-gnu \
    -e features > "$out/dependency-features.txt"
cargo +1.88.0 build --locked -p smallvil 2>&1 | tee "$out/build.log"
test -x target/debug/smallvil
cmp Cargo.lock "$out/upstream-Cargo.lock"
