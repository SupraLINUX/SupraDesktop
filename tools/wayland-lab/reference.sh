#!/usr/bin/env bash
# External reference compositor only; no host display, input or session changes.
set -euo pipefail
out=$(realpath -m "$1")
mkdir -p "$out"
runtime=$(mktemp -d)
chmod 0700 "$runtime"
server=
cleanup() {
    if [[ -n "$server" ]]; then
        kill "$server" 2>/dev/null || true
        wait "$server" 2>/dev/null || true
    fi
    rm -rf -- "$runtime"
}
trap cleanup EXIT
unset DISPLAY WAYLAND_SOCKET WAYLAND_DEBUG
export XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=supra-reference
dpkg-query -W weston wayland-utils > "$out/versions.txt"
weston --version >> "$out/versions.txt"
weston --backend=headless-backend.so --use-pixman --socket="$WAYLAND_DISPLAY" \
    --width=800 --height=600 --idle-time=0 --no-config --log="$out/weston.log" &
server=$!
for _ in $(seq 1 100); do
    kill -0 "$server"
    [[ -S "$runtime/$WAYLAND_DISPLAY" ]] && break
    sleep 0.1
done
test -S "$runtime/$WAYLAND_DISPLAY"
timeout 10 wayland-info > "$out/globals.txt"
status=0
WAYLAND_DEBUG=client timeout --signal=TERM --kill-after=3 5 weston-simple-shm \
    > "$out/client.stdout" 2> "$out/client.trace" || status=$?
# The animated client is intentionally stopped after a bounded interval.
if [[ "$status" != 124 && "$status" != 0 ]]; then exit "$status"; fi
kill -0 "$server"
python3 tools/wayland-lab/trace.py "$out"
