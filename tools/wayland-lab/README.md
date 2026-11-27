# Wayland reference lab

Run in disposable Ubuntu CI, with packaged Weston and wayland-utils only. See [Weston manual](https://manpages.ubuntu.com/manpages/noble/man1/weston.1.html) and [Weston running documentation](https://mvlad.pages.freedesktop.org/weston/toc/running-weston.html).

`bash tools/wayland-lab/reference.sh evidence` creates a private XDG_RUNTIME_DIR and headless software compositor, checks advertised globals and runs the packaged weston-simple-shm client with WAYLAND_DEBUG. The validator tracks one xdg surface, its configure acknowledgment, attached buffer, surface commit and frame callback, without requiring a real display or touching the host session. Outputs package versions, globals, client/server logs and JSON. CI installs tools as test-only dependencies; no external source is copied. This is an external reference baseline, not an own compositor, GPU validation or conformance suite.
