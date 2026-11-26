"""Numerical left/right fixture comparisons; encoded RGB, not native fidelity."""
import argparse
import json
import math
from pathlib import Path
import re


def read_ppm(path):
    data = path.read_bytes()
    header = re.match(rb"P6\n(\d+) (\d+)\n255\n", data)
    if not header:
        raise ValueError("Expected laboratory P6 format")
    w, h = map(int, header.groups())
    payload = data[header.end():]
    if w <= 0 or h <= 0 or len(payload) != w * h * 3:
        raise ValueError("Invalid image dimensions/payload")
    return w, h, payload


def summarize(hist, count, changed, channel_sums):
    if count <= 0:
        raise ValueError("Empty comparison region")
    total = sum(i * n for i, n in enumerate(hist))
    threshold = math.ceil(0.95 * count * 3)
    cumulative = 0
    p95 = 0
    for i, n in enumerate(hist):
        cumulative += n
        if cumulative >= threshold:
            p95 = i
            break
    return {"pixels": count, "changed_pixels": changed,
            "mean_absolute_channel_error": total / (count * 3),
            "p95_absolute_channel_error": p95,
            "max_absolute_channel_error": max(i for i, n in enumerate(hist) if n),
            "mean_absolute_error_rgb": [v / count for v in channel_sums]}


def compare(w, h, payload, roi):
    if w <= 0 or h <= 0 or w % 2 or len(payload) != w * h * 3:
        raise ValueError("Expected equal-width comparison panels")
    half = w // 2
    x0, y0, x1, y1 = roi
    if not (0 <= x0 < x1 <= half and 0 <= y0 < y1 <= h):
        raise ValueError("ROI outside panel")
    regions = {k: {"hist": [0] * 256, "count": 0, "changed": 0, "sums": [0] * 3}
               for k in ("inside_roi", "outside_roi")}
    for y in range(h):
        for x in range(half):
            r = regions["inside_roi" if x0 <= x < x1 and y0 <= y < y1 else "outside_roi"]
            a, b = (y * w + x) * 3, (y * w + x + half) * 3
            diffs = [abs(payload[a+c] - payload[b+c]) for c in range(3)]
            r["count"] += 1
            r["changed"] += int(any(diffs))
            for c, d in enumerate(diffs):
                r["hist"][d] += 1
                r["sums"][c] += d
    result = {k: summarize(r["hist"], r["count"], r["changed"], r["sums"])
              for k, r in regions.items()}
    return result


def validate(result):
    if result["outside_roi"]["changed_pixels"]:
        raise ValueError("Material leaked outside declared bounding ROI")
    if not result["inside_roi"]["changed_pixels"]:
        raise ValueError("No optical effect detected inside ROI")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("evidence", type=Path)
    a = p.parse_args()
    roi = (48, 108, 272, 252)
    fixtures = {}
    for name in ("refraction", "material"):
        w, h, payload = read_ppm(a.evidence / f"{name}.ppm")
        if (w, h) != (640, 360):
            raise ValueError("Unexpected fixture size")
        result = compare(w, h, payload, roi)
        validate(result)
        fixtures[name] = result
    report = {"schema_version": 1, "kind": "synthetic-optics-locality",
              "encoding": "8-bit displayed RGB, not linear luminance",
              "roi": list(roi), "roi_definition": "shape bounding rectangle in each panel",
              "macos_calibrated": False, "fixtures": fixtures}
    (a.evidence / "optics-metrics.json").write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
