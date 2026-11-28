"""Validate benchmark evidence and summarize each same-run sample group."""
import csv
import json
import statistics
import sys
from pathlib import Path

def summarize(path):
    groups = {}
    with path.open(newline="") as source:
        for row in csv.DictReader(source):
            assert row["pixel_exact"] == "true"
            assert int(row["frames"]) == 24
            key = (row["scenario"], int(row["chunk_pixels"]))
            groups.setdefault(key, []).append(row)
    assert set(groups) == {(s, n) for s in ("movement", "clipped")
                           for n in (16, 32, 64, 128, 256, 512)}
    result = []
    for (scenario, chunk), rows in sorted(groups.items()):
        assert sorted(int(r["repeat"]) for r in rows) == list(range(7))
        ns = [int(r["update_ns"]) for r in rows]
        assert min(ns) > 0
        counts = {}
        for key in ("cow_copied_pixels", "published_pixels", "prefix_pixels"):
            values = {int(r[key]) for r in rows}
            assert len(values) == 1 and min(values) >= 0
            counts[key] = values.pop()
        result.append(dict(scenario=scenario, chunk_pixels=chunk,
                           median_update_ns=statistics.median(ns),
                           min_update_ns=min(ns), max_update_ns=max(ns),
                           repeats=len(ns), **counts))
    return {"schema_version": 1, "pixel_exact": True, "groups": result,
            "timing_scope": "sum of update calls per 24-frame sequence",
            "allocation_count_measured": False, "gpu_measured": False}

if __name__ == "__main__":
    print(json.dumps(summarize(Path(sys.argv[1])), indent=2))
