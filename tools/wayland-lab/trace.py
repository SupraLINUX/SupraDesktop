"""Check a real client trace, using per-object configure/commit/frame evidence."""
import json
from pathlib import Path
import re
import sys

EVENT = re.compile(r"\b([a-z_0-9]+)[#@](\d+)\.([a-z_0-9]+)\((.*)\)")
REF = r"{}[#@](\d+)"


def object_id(args, interface):
    match = re.search(REF.format(interface), args)
    return int(match.group(1)) if match else None


def validate(globals_text, trace):
    for interface in ("wl_compositor", "wl_shm", "xdg_wm_base"):
        if not re.search(r"interface:\s*['\"]" + interface + r"['\"]", globals_text):
            raise ValueError(f"Missing advertised global: {interface}")
    surfaces = {}
    toplevels = set()
    configure = {}
    acknowledged = set()
    attached = set()
    committed = set()
    callbacks = {}
    frames = set()
    for line in trace.splitlines():
        match = EVENT.search(line)
        if not match:
            continue
        interface, number, event, args = match.groups()
        oid = int(number)
        if interface == "wl_display" and event == "error":
            raise ValueError("Wayland protocol error")
        if interface == "xdg_wm_base" and event == "get_xdg_surface":
            xdg, surface = object_id(args, "xdg_surface"), object_id(args, "wl_surface")
            if xdg is not None and surface is not None:
                surfaces[xdg] = surface
        elif interface == "xdg_surface" and event == "get_toplevel" and oid in surfaces:
            toplevels.add(oid)
        elif interface == "xdg_surface" and event == "configure" and oid in toplevels:
            configure.setdefault(oid, set()).add(args.strip())
        elif interface == "xdg_surface" and event == "ack_configure":
            if oid in toplevels and args.strip() in configure.get(oid, set()):
                acknowledged.add(surfaces[oid])
        elif interface == "wl_surface" and event == "attach":
            if oid in acknowledged and object_id(args, "wl_buffer") is not None:
                attached.add(oid)
        elif interface == "wl_surface" and event == "commit" and oid in attached:
            committed.add(oid)
        elif interface == "wl_surface" and event == "frame":
            cb = object_id(args, "wl_callback")
            if cb is not None:
                callbacks[cb] = oid
        elif interface == "wl_callback" and event == "done":
            surface = callbacks.pop(oid, None)
            if surface in committed:
                frames.add(surface)
    if not frames:
        raise ValueError("No configured toplevel with acknowledged buffer commit and frame callback")
    return {"schema_version": 1, "kind": "external-wayland-reference",
            "required_globals": True, "configured_committed_frame": True,
            "verified_surfaces": len(frames), "supra_compositor_verified": False,
            "gpu_verified": False}


if __name__ == "__main__":
    out = Path(sys.argv[1])
    report = validate((out / "globals.txt").read_text(),
                      (out / "client.trace").read_text(errors="replace"))
    (out / "wayland-report.json").write_text(json.dumps(report, sort_keys=True) + "\n")
    print(json.dumps(report, sort_keys=True))
