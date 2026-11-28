"""Compare independent host capture pixels to original rendered RGB.
No crop coordinates are assumed; require a unique exact unscaled rectangle.
"""
import hashlib
import json
import sys
from pathlib import Path
from PIL import Image


def locate(host, source):
    host = host.convert("RGB")
    source = source.convert("RGB")
    w, h = host.size
    sw, sh = source.size
    if sw > w or sh > h or sw < 1 or sh < 1:
        raise ValueError("Source does not fit capture")
    host_rows = [host.crop((0, y, w, y+1)).tobytes() for y in range(h)]
    src_rows = [source.crop((0, y, sw, y+1)).tobytes() for y in range(sh)]
    # Exact central row is an inexpensive prefilter; every pixel is then
    # compared independently. No resize, tolerance, or channel correction.
    anchor = sh//2
    needle = src_rows[anchor]
    stride = 3*sw
    matches = []
    for y in range(h-sh+1):
        row = host_rows[y+anchor]
        start = 0
        while True:
            p = row.find(needle, start)
            if p < 0:
                break
            start = p+1
            if p % 3 or p+stride > 3*w:
                continue
            if all(host_rows[y+dy][p:p+stride] == s for dy, s in enumerate(src_rows)):
                matches.append((p//3,y))
                if len(matches) > 1:
                    raise ValueError("Ambiguous exact source rectangle")
    if len(matches) != 1:
        raise ValueError("No exact source rectangle in host readback")
    return matches[0]


def compare(first, second):
    locations = []
    results = []
    for host, source in (first, second):
        if host.size != (800,600) or source.size != (160,96):
            raise ValueError("Unexpected capture or source size")
        x,y = locate(host, source)
        locations.append((x,y))
        data = host.convert("RGB").crop((x,y,x+160,y+96)).tobytes()
        expected = source.convert("RGB").tobytes()
        if data != expected:
            raise ValueError("Pixel mismatch")
        results.append({"x": x, "y": y, "pixels": 160*96,
                        "different_pixels": 0, "rgb_sha256": hashlib.sha256(data).hexdigest()})
    if locations[0] != locations[1]:
        raise ValueError("Surface moved between readbacks")
    if results[0]["rgb_sha256"] == results[1]["rgb_sha256"]:
        raise ValueError("Frames must differ")
    return results


if __name__ == "__main__":
    out = Path(sys.argv[1])
    frames = []
    for n in (1,2):
        with Image.open(out / f"host-frame-{n}.png") as host, Image.open(out / f"nested-frame-{n}.ppm") as source:
            if host.format != "PNG" or source.format != "PPM":
                raise ValueError("Unexpected capture format")
            frames.append((host.copy(),source.copy()))
    report = {"schema_version": 1, "host": "weston-headless-pixman",
              "independent_host_capture": True, "frames": compare(*frames),
              "physical_presentation_verified": False, "gpu_verified": False,
              "own_wayland_server_verified": False}
    data = json.dumps(report, sort_keys=True) + "\n"
    (out / "host-readback-report.json").write_text(data)
    print(data, end="")
