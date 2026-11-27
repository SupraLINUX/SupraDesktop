"""Inventory actual license files for the exact external candidate, not clearance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib


def parse_tree(text):
    entries = {}
    for line in text.splitlines():
        package, sep, license_text = line.partition("|")
        match = re.fullmatch(r"([A-Za-z0-9_-]+) v([A-Za-z0-9.+-]+)(?: \([^\n]*\))?", package)
        if not sep or not match:
            raise ValueError("Unrecognized cargo tree line: " + line)
        key = match.groups()
        license_text = license_text.removesuffix(" (*)")
        if key in entries and entries[key] != license_text:
            raise ValueError("Inconsistent declared license for " + str(key))
        entries[key] = license_text
    if not entries:
        raise ValueError("Empty dependency inventory")
    return entries


def package_dir(upstream, cargo_home, name, version):
    if name in ("smithay", "smallvil"):
        candidate = upstream if name == "smithay" else upstream / "smallvil"
        candidates = [candidate]
    else:
        candidates = sorted((cargo_home / "registry" / "src").glob(f"*/{name}-{version}"))
    if len(candidates) != 1:
        raise ValueError(f"Ambiguous or missing source directory: {name} {version}")
    root = candidates[0].resolve()
    manifest = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    if manifest.get("name") != name or manifest.get("version") != version:
        raise ValueError(f"Unexpected manifest identity: {name} {version}")
    return root, manifest


def license_paths(root, manifest):
    candidates = set()
    for path in root.iterdir():
        upper = path.name.upper()
        if upper.startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")):
            if path.is_file():
                candidates.add(path)
            elif path.is_dir() and not path.is_symlink():
                candidates.update(p for p in path.rglob("*") if p.is_file())
    explicit = manifest.get("license-file")
    if isinstance(explicit, str):
        candidates.add(root / explicit)
    for p in sorted(candidates):
        if p.is_symlink() or not p.resolve().is_relative_to(root) or not p.is_file():
            raise ValueError(f"Unsafe or missing license path: {p}")
        yield p


def main():
    p = argparse.ArgumentParser()
    p.add_argument("evidence", type=Path)
    p.add_argument("upstream", type=Path)
    a = p.parse_args()
    out = a.evidence.resolve()
    upstream = a.upstream.resolve()
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    entries = parse_tree((out / "dependency-licenses.txt").read_text())
    report = []
    for (name, version), expression in sorted(entries.items()):
        root, manifest = package_dir(upstream, cargo_home, name, version)
        item = {"name": name, "version": version, "declared": expression, "files": []}
        for path in license_paths(root, manifest):
            relative = path.relative_to(root)
            data = path.read_bytes()
            if len(data) > 1024 * 1024:
                raise ValueError(f"License file too large for evidence: {relative}")
            # Preserve the actual texts separately from package source code.
            dest = out / "license-texts" / f"{name}-{version}" / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, dest)
            item["files"].append({"path": str(relative), "sha256": hashlib.sha256(data).hexdigest(),
                                  "bytes": len(data)})
        report.append(item)
    native = subprocess.run(["ldd", str(upstream / "target/debug/smallvil")],
                            check=True, text=True, capture_output=True).stdout
    (out / "native-ldd.txt").write_text(native)
    if "not found" in native:
        raise ValueError("Unresolved dynamic dependency")
    packages = []
    for line in native.splitlines():
        match = re.search(r"(?:=>\s+|^\s*)(/[^\s]+)", line)
        if not match:
            continue
        path = Path(match.group(1))
        candidates = sorted({str(path), str(path.resolve())})
        found = []
        for candidate in candidates:
            result = subprocess.run(["dpkg-query", "-S", candidate], text=True, capture_output=True)
            if result.returncode == 0:
                found.extend(result.stdout.splitlines())
        packages.append({"path": str(path), "package_owners": sorted(set(found))})
    summary = {"schema_version": 1, "target": "x86_64-unknown-linux-gnu",
               "candidate": "external-smallvil", "packages": report,
               "native_libraries": packages, "license_audit_complete": False,
               "dynamic_loading_audited": False, "dependency_adopted": False}
    (out / "license-file-inventory.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"package_count": len(report),
                      "declared_missing": [x["name"] for x in report if not x["declared"]],
                      "files_missing": [x["name"] for x in report if not x["files"]],
                      "native_unmapped": [x["path"] for x in packages if not x["package_owners"]],
                      "license_audit_complete": False}, sort_keys=True))


if __name__ == "__main__":
    main()
