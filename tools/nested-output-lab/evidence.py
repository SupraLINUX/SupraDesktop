"""Validate actual host trace and renderer boundary, not physical presentation."""
import json
from pathlib import Path
import re
import hashlib
import sys

EVENT = re.compile(r"\b([a-z_0-9]+)[#@](\d+)\.([a-z_0-9]+)\((.*)\)")


def verify(trace, renderer):
    # IDs are reusable across object lifetimes. Track live callback purpose,
    # not membership in a historical set; delete_id can precede done in a
    # libwayland debug trace, so consume on the typed event itself.
    callbacks = {}
    buffers = {}
    surfaces = {}
    xdg = {}
    configured = {}
    acknowledged = set()
    frames = 0
    releases = 0
    def ref(args, interface):
        m = re.search(r"\b" + interface + r"[#@](\d+)", args)
        if not m:
            raise ValueError(f"Missing {interface} reference")
        return m[1]
    for line in trace.splitlines():
        match = EVENT.search(line)
        if not match:
            continue
        interface, oid, event, args = match.groups()
        request = "->" in line[:match.start()]
        if interface == "wl_display" and event == "error":
            raise ValueError("Host protocol error")
        if request and interface == "wl_compositor" and event == "create_surface":
            surfaces[ref(args, "wl_surface")] = {"attach": None, "frame": None, "commits": 0}
        elif request and interface == "xdg_wm_base" and event == "get_xdg_surface":
            xdg[ref(args, "xdg_surface")] = ref(args, "wl_surface")
        elif not request and interface == "xdg_surface" and event == "configure":
            configured[oid] = args.strip()
        elif request and interface == "xdg_surface" and event == "ack_configure":
            if configured.get(oid) != args.strip() or oid not in xdg:
                raise ValueError("Unmatched configure")
            acknowledged.add(xdg[oid])
        elif request and interface == "wl_shm_pool" and event == "create_buffer":
            b = ref(args, "wl_buffer")
            if b in buffers:
                raise ValueError("Duplicate buffer lifetime")
            if not re.search(r",\s*0,\s*160,\s*96,\s*640,\s*1$", args):
                raise ValueError("Unexpected buffer layout")
            buffers[b] = "created"
        elif request and ((interface == "wl_surface" and event == "frame") or
                          (interface == "wl_display" and event == "sync")):
            cb = ref(args, "wl_callback")
            if cb in callbacks:
                raise ValueError("Overlapping callback lifetime")
            if interface == "wl_display":
                callbacks[cb] = ("sync", None)
            else:
                s = surfaces[oid]
                if s["frame"] is not None:
                    raise ValueError("Overlapping surface frame")
                s["frame"] = cb
                callbacks[cb] = ("pending", oid)
        elif request and interface == "wl_surface" and event == "attach":
            s = surfaces[oid]
            if args.startswith("nil") or args.startswith("null"):
                s["attach"] = None
            else:
                b = ref(args, "wl_buffer")
                if oid not in acknowledged or buffers.get(b) != "created":
                    raise ValueError("Unconfigured or invalid buffer attach")
                s["attach"] = b
        elif request and interface == "wl_surface" and event == "commit":
            s = surfaces[oid]
            s["commits"] += 1
            b, cb = s["attach"], s["frame"]
            if b is not None:
                if cb is None or buffers.get(b) != "created":
                    raise ValueError("Buffer commit without new frame request")
                buffers[b] = "committed"
                callbacks[cb] = ("committed", oid)
            elif cb is not None:
                raise ValueError("Frame without buffer")
        elif not request and interface == "wl_callback" and event == "done":
            kind, surface = callbacks.pop(oid, ("unknown", None))
            if kind == "sync":
                continue
            if kind != "committed" or surfaces[surface]["frame"] != oid:
                raise ValueError("Callback without committed frame")
            surfaces[surface]["frame"] = None
            frames += 1
        elif not request and interface == "wl_buffer" and event == "release":
            if buffers.get(oid) != "committed":
                raise ValueError("Release before commit or duplicate release")
            buffers[oid] = "released"
            releases += 1
        elif request and interface == "wl_buffer" and event == "destroy":
            if buffers.get(oid) != "released":
                raise ValueError("Buffer destroyed before release")
            buffers[oid] = "destroyed"
    if (frames != 2 or releases != 2 or len(buffers) != 2 or
        any(v != "destroyed" for v in buffers.values()) or callbacks or
        len(surfaces) != 1 or next(iter(surfaces.values()))["commits"] != 4):
        raise ValueError(f"Incomplete host lifecycle: frames={frames}, releases={releases}, buffers={buffers}, callbacks={callbacks}")
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
