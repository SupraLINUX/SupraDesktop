#!/usr/bin/env bash
# Private, synthetic CI output only. This script is not a desktop feature.
set -euo pipefail
out=$(realpath "$1")
case "$2" in 1|2) ;; *) exit 1 ;; esac
test "$WAYLAND_DISPLAY" = supra-optical-output
test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
test "$(stat -c %a "$XDG_RUNTIME_DIR")" = 700
dir="$out/capture-$2"
mkdir "$dir"
(
    cd "$dir"
    export XDG_PICTURES_DIR="$dir"
    timeout --kill-after=2 12 weston-screenshooter > capture.stdout 2> capture.stderr
)
shopt -s nullglob
images=("$dir"/*.png)
test "${#images[@]}" = 1
test -s "${images[0]}"
mv -- "${images[0]}" "$out/host-frame-$2.png"
