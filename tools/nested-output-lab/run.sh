#!/usr/bin/env bash
# CI-only external output smoke test; no host session or global socket.
set -euo pipefail
out=$(realpath -m "$1")
mkdir -p "$out"
runtime=$(mktemp -d)
chmod 0700 "$runtime"
server=
cleanup() {
    if [[ -n "$server" ]]; then kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; fi
    rm -rf -- "$runtime"
}
trap cleanup EXIT
unset DISPLAY WAYLAND_SOCKET WAYLAND_DEBUG
export XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=supra-optical-output
mode=${2:-transport}
case "$mode" in transport|readback) ;; *) exit 1 ;; esac
weston_flags=()
unset SUPRA_LAB_CAPTURE_SCRIPT
if [[ "$mode" = readback ]]; then
    # Debug capture is enabled exclusively on the isolated CI compositor.
    weston_flags=(--debug)
    export SUPRA_LAB_CAPTURE_SCRIPT="$(realpath tools/nested-output-lab/capture.sh)"
    command -v weston-screenshooter
fi
protocol="$(pkg-config --variable=pkgdatadir wayland-protocols)/stable/xdg-shell/xdg-shell.xml"
wayland-scanner client-header "$protocol" "$runtime/xdg-shell-client-protocol.h"
wayland-scanner private-code "$protocol" "$runtime/xdg-shell-protocol.c"
cc -std=c11 -Wall -Wextra -Werror -I"$runtime" tools/nested-output-lab/client.c "$runtime/xdg-shell-protocol.c" \
    $(pkg-config --cflags --libs wayland-client) -o "$runtime/client"
dpkg-query -W weston libwayland-dev wayland-protocols > "$out/versions.txt"
weston --backend=headless-backend.so --use-pixman --socket="$WAYLAND_DISPLAY" \
    --width=800 --height=600 --idle-time=0 --no-config --log="$out/weston.log" "${weston_flags[@]}" &
server=$!
for _ in $(seq 1 100); do
    kill -0 "$server"
    [[ -S "$runtime/$WAYLAND_DISPLAY" ]] && break
    sleep 0.1
done
test -S "$runtime/$WAYLAND_DISPLAY"
cargo build --locked -p supra-lab
WAYLAND_DEBUG=client timeout --kill-after=5 60 target/debug/supra-lab --nested-output "$runtime/client" "$out" \
    > "$out/renderer.log" 2> "$out/client.trace"
kill -0 "$server"
python3 tools/nested-output-lab/evidence.py "$out"

if [[ "$mode" = readback ]]; then
    /usr/bin/python3 tools/nested-output-lab/readback.py "$out"
fi
