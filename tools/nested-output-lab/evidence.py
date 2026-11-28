"""Validate actual host trace and renderer boundary, not physical presentation."""
import json
from pathlib import Path
import re
import hashlib
import sys

EVENT = re.compile(r"\b([a-z_0-9]+)[#@](\d+)\.([a-z_0-9]+)\((.*)\)")


def verify(trace, renderer):
    requests = []
    done = set()
    buffers = set()
    releases = set()
    attached = set()
    commits = 0
    for line in trace.splitlines():
        match = EVENT.search(line)
        if not match:
            continue
        interface, oid, event, args = match.groups()
        if interface == "wl_display" and event == "error":
            raise ValueError("Host protocol error")
        if interface == "wl_shm_pool" and event == "create_buffer":
            b = re.search(r"wl_buffer[#@](\d+)", args)
            if b:
                buffers.add(b[1])
        if interface == "wl_surface" and event == "attach":
            b = re.search(r"wl_buffer[#@](\d+)", args)
            if b:
                attached.add(b[1])
        if interface == "wl_surface" and event == "commit":
            commits += 1
        if interface == "wl_surface" and event == "frame":
            c = re.search(r"wl_callback[#@](\d+)", args)
            if c:
                requests.append(c[1])
        if interface == "wl_callback" and event == "done" and oid in requests:
            done.add(oid)
        if interface == "wl_buffer" and event == "release":
            releases.add(oid)
    # Callback object IDs can be recycled; count done events as well.
    callbacks = [EVENT.search(line) for line in trace.splitlines()]
    matching = sum(1 for m in callbacks if m and m[1] == "wl_callback" and m[3] == "done" and m[2] in requests)
    if len(requests) != 2 or matching != 2 or not done or len(buffers) != 2 or attached != buffers or releases != buffers or commits < 4:
        raise ValueError("Incomplete real host frame/buffer lifecycle")
    if renderer.splitlines() != ["SUPRA_NESTED_FRAME_1_COMPLETE", "SUPRA_NESTED_FRAME_2_COMPLETE", "SUPRA_NESTED_BUFFERS_RELEASED"]:
        raise ValueError("Invalid renderer completion sequence")


if __name__ == "__main__":
    out = Path(sys.argv[1])
    verify((out / "client.trace").read_text(errors="replace"), (out / "renderer.log").read_text())
    hashes = []
    for n in (1, 2):
        pixels = (out / f"nested-frame-{n}.ppm").read_bytes()
        header = b"P6\n160 96\n255\n"
        if not pixels.startswith(header) or len(pixels) != len(header) + 160*96*3:
            raise ValueError("Invalid renderer fixture")
        hashes.append(hashlib.sha256(pixels).hexdigest())
    if hashes[0] == hashes[1]:
        raise ValueError("Expected changed optical frame")
    report = {"schema_version": 1, "renderer": "supra-optical-cpu", "host": "weston-headless-pixman",
              "host_frame_callbacks": 2, "host_buffer_releases": 2, "frame_sha256": hashes,
              "supra_callback_completion_verified": True, "own_wayland_server_verified": False,
              "physical_presentation_verified": False, "gpu_verified": False}
    (out / "nested-output-report.json").write_text(json.dumps(report, sort_keys=True) + "\n")
    print(json.dumps(report, sort_keys=True))
