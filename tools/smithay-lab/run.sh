#!/usr/bin/env bash
# Isolated nested reference: Weston -> unmodified upstream Smallvil -> SHM client.
set -euo pipefail
out=$(realpath -m "$1")
runtime=$(mktemp -d)
chmod 0700 "$runtime"
outer=
inner=
cleanup() {
    for pid in "$inner" "$outer"; do
        if [[ -n "$pid" ]]; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    rm -rf -- "$runtime"
}
trap cleanup EXIT
unset DISPLAY WAYLAND_SOCKET WAYLAND_DEBUG
export XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=supra-outer
export LIBGL_ALWAYS_SOFTWARE=1
weston --backend=headless-backend.so --use-pixman --socket="$WAYLAND_DISPLAY" \
    --width=1280 --height=720 --idle-time=0 --no-config --log="$out/outer.log" &
outer=$!
for _ in $(seq 1 100); do
    kill -0 "$outer"
    [[ -S "$runtime/$WAYLAND_DISPLAY" ]] && break
    sleep 0.1
done
test -S "$runtime/$WAYLAND_DISPLAY"
# /usr/bin/true prevents the sample's default terminal launch.
RUST_LOG=info "$RUNNER_TEMP/smithay-candidate/target/debug/smallvil" -c /usr/bin/true \
    > "$out/smallvil.log" 2>&1 &
inner=$!
socket=
for _ in $(seq 1 200); do
    kill -0 "$inner"
    for candidate in "$runtime"/wayland-*; do
        if [[ -S "$candidate" ]]; then socket="${candidate##*/}"; break; fi
    done
    if [[ -n "$socket" ]] && WAYLAND_DISPLAY="$socket" timeout 2 wayland-info > "$out/globals.txt" 2>> "$out/client-probe.log"; then
        break
    fi
    sleep 0.1
done
test -n "$socket"
export WAYLAND_DISPLAY="$socket"
timeout 10 wayland-info > "$out/globals.txt"
status=0
WAYLAND_DEBUG=client timeout --signal=TERM --kill-after=3 5 weston-simple-shm \
    > "$out/client.stdout" 2> "$out/client.trace" || status=$?
if [[ "$status" != 124 && "$status" != 0 ]]; then exit "$status"; fi
kill -0 "$outer"
kill -0 "$inner"
python3 tools/wayland-lab/trace.py "$out"
python3 - "$out" <<'PY'
import json
from pathlib import Path
import sys
out = Path(sys.argv[1])
trace = json.loads((out / "wayland-report.json").read_text())
if trace.get("configured_committed_frame") is not True:
    raise ValueError("Missing nested client evidence")
result = {"schema_version": 1, "kind": "upstream-smithay-candidate",
          "upstream_commit": (out / "upstream-commit.txt").read_text().strip(),
          "nested_client_verified": True, "software_rendering_requested": True,
          "supra_compositor_verified": False, "hardware_gpu_verified": False,
          "dependency_adopted": False, "license_audit_complete": False}
(out / "smithay-report.json").write_text(json.dumps(result, sort_keys=True) + "\n")
print(json.dumps(result, sort_keys=True))
PY
