"""Observe actual mapped libraries of our external, isolated lab process."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def shared_paths(maps):
    paths = set()
    for line in maps.splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) == 6:
            name = fields[5]
            # Only packaged shared libraries, not arbitrary mapped data/files.
            if name.startswith(("/usr/lib/", "/lib/", "/lib64/", "/usr/lib64/")):
                if re.search(r"\.so(?:\.[^/ ]+)?$", name):
                    paths.add(name)
    return sorted(paths)


def owners(path):
    names = set()
    for candidate in sorted({str(path), str(path.resolve())}):
        result = subprocess.run(["dpkg-query", "-S", candidate],
                                text=True, capture_output=True, check=False)
        if result.returncode == 0:
            for line in result.stdout.splitlines():
                prefix, sep, _ = line.partition(": ")
                if sep and not prefix.startswith("diversion"):
                    for name in prefix.split(", "):
                        if re.fullmatch(r"[a-z0-9][a-z0-9+.-]*(?::[a-z0-9]+)?", name):
                            names.add(name)
    return sorted(names)


def native_notice(out, name):
    version = subprocess.run(["dpkg-query", "-W", "-f=${Version}", name],
                             text=True, capture_output=True, check=True).stdout
    notice = Path("/usr/share/doc") / name.split(":")[0] / "copyright"
    result = {"package": name, "version": version, "notice_present": notice.is_file()}
    if notice.is_file():
        data = notice.read_bytes()
        target = out / "native-notices" / name.replace(":", "_") / "copyright"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        result.update({"notice_sha256": hashlib.sha256(data).hexdigest(),
                       "notice_bytes": len(data)})
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence", type=Path)
    parser.add_argument("pid", type=int)
    args = parser.parse_args()
    out = args.evidence
    proc = Path("/proc") / str(args.pid)
    if (proc / "comm").read_text().strip() != "smallvil":
        raise ValueError("Not the expected isolated sample process")
    paths = shared_paths((proc / "maps").read_text())
    if not paths:
        raise ValueError("No mapped native libraries")
    libraries = [{"path": p, "package_owners": owners(Path(p))} for p in paths]
    names = sorted({name for lib in libraries for name in lib["package_owners"]})
    report = {"schema_version": 1, "scope": "single-live-smallvil-process-sample",
              "libraries": libraries, "packages": [native_notice(out, n) for n in names],
              "unmapped": [lib["path"] for lib in libraries if not lib["package_owners"]],
              "all_dynamic_loading_audited": False, "license_audit_complete": False}
    (out / "native-runtime-inventory.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"mapped_library_count": len(libraries), "package_count": len(names),
                      "unmapped": report["unmapped"],
                      "notices_missing": [p["package"] for p in report["packages"] if not p["notice_present"]],
                      "license_audit_complete": False}, sort_keys=True))


if __name__ == "__main__":
    main()
