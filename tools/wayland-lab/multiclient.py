"""Run independent real Wayland clients on the already isolated lab socket."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from trace import EVENT, validate


def callbacks(trace):
    count = 0
    for line in trace.splitlines():
        match = EVENT.search(line)
        if match:
            interface, _, event, _ = match.groups()
            if interface == "wl_display" and event == "error":
                raise ValueError("Client protocol error")
            if interface == "wl_callback" and event == "done":
                count += 1
    return count


def read(path):
    return path.read_text(errors="replace")


def check_live(process):
    if process.poll() is not None:
        raise RuntimeError(f"Client exited before requested termination: {process.returncode}")


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=3)


def ready(globals_text, path):
    try:
        return validate(globals_text, read(path))["configured_committed_frame"]
    except ValueError:
        return False


def wait_until(predicate, processes, seconds=12):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        for process in processes:
            check_live(process)
        if predicate():
            return
        time.sleep(0.05)
    raise TimeoutError("Client evidence deadline exceeded")


def report(globals_text, first, survivor, before):
    validate(globals_text, first)
    validate(globals_text, survivor)
    final_count = callbacks(survivor)
    if before < 1 or final_count <= before:
        raise ValueError("Survivor has no confirmed callbacks after peer exit")
    return {"schema_version": 1, "kind": "external-wayland-client-coexistence",
            "concurrent_clients": 2, "both_configured_committed_frame": True,
            "survivor_callbacks_before": before, "survivor_callbacks_final": final_count,
            "survivor_progress_after_peer_exit": True,
            "security_isolation_verified": False, "supra_compositor_verified": False}


def main(out):
    globals_text = read(out / "globals.txt")
    paths = [out / "multi-first.trace", out / "multi-survivor.trace"]
    processes = []
    env = dict(os.environ, WAYLAND_DEBUG="client")
    # Guard against accidentally connecting to the desktop outside this lab.
    runtime = Path(env["XDG_RUNTIME_DIR"])
    socket = env["WAYLAND_DISPLAY"]
    if not runtime.is_dir() or runtime.stat().st_mode & 0o077 or "/" in socket:
        raise ValueError("Expected isolated runtime and relative Wayland socket")
    if not (runtime / socket).is_socket():
        raise ValueError("Missing lab socket")
    try:
        for path in paths:
            with path.open("wb") as trace:
                processes.append(subprocess.Popen(["weston-simple-shm"], env=env,
                                                  stdout=subprocess.DEVNULL, stderr=trace))
        wait_until(lambda: all(ready(globals_text, p) for p in paths), processes)
        # Snapshot the survivor count only AFTER the first process exits.
        stop(processes[0])
        check_live(processes[1])
        before = callbacks(read(paths[1]))
        wait_until(lambda: callbacks(read(paths[1])) >= before + 3, [processes[1]])
        stop(processes[1])
        result = report(globals_text, read(paths[0]), read(paths[1]), before)
        (out / "multiclient-report.json").write_text(json.dumps(result, sort_keys=True) + "\n")
        print(json.dumps(result, sort_keys=True))
    finally:
        for process in processes:
            stop(process)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
