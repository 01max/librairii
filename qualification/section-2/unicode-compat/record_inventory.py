#!/usr/bin/env python3
"""Read verified cached registry archives; record this experiment's graph/notices."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tomllib

HERE = Path(__file__).resolve().parent


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-home", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--features", type=Path, required=True)
    parser.add_argument("--text-cache", type=Path, required=True)
    args = parser.parse_args()
    metadata = json.loads(args.metadata.read_bytes())
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    selected = {p["name"]: (p, nodes.get(p["id"])) for p in metadata["packages"] if p["source"]}
    lock = tomllib.loads((HERE / "Cargo.lock").read_text())
    rows = []
    notices = HERE / "notices"
    notices.mkdir(exist_ok=True)
    for package in lock["package"]:
        if "source" not in package:
            continue
        name, version = package["name"], package["version"]
        matches = list((args.cargo_home / "registry/cache").glob(f"*/{name}-{version}.crate"))
        if len(matches) != 1:
            raise RuntimeError(f"expected one cached source archive for {name}-{version}")
        archive = matches[0].read_bytes()
        if sha(archive) != package["checksum"]:
            raise RuntimeError("registry source checksum mismatch")
        root = f"{name}-{version}/"
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
            manifest = tar.extractfile(root + "Cargo.toml").read()
            source = tomllib.loads(manifest.decode())
            vcs = json.loads(tar.extractfile(root + ".cargo_vcs_info.json").read())
            row = {"name": name, "version": version, "source": package["source"],
                   "archiveSha256": package["checksum"], "manifestSha256": sha(manifest),
                   "license": source["package"].get("license"),
                   "rustVersionDeclared": source["package"].get("rust-version"),
                   "repository": source["package"].get("repository"), "publishedVcs": vcs,
                   "selectedInHostMetadata": name in selected,
                   "features": selected[name][1]["features"] if name in selected else [],
                   "notices": [], "candidateSources": {}}
            for member in tar.getmembers():
                path = member.name.removeprefix(root)
                if not member.isfile():
                    continue
                if "/" not in path and any(word in path.upper() for word in ["LICENSE", "COPYRIGHT", "NOTICE", "UNLICENSE"]):
                    data = tar.extractfile(member).read()
                    destination = notices / (sha(data) + ".txt")
                    destination.write_bytes(data)
                    row["notices"].append({"sourcePath": path, "sha256": sha(data),
                                           "file": str(destination.relative_to(HERE))})
                if name in ["unicode-normalization", "unicode-general-category", "tinyvec"] and path in ["src/lib.rs", "src/tables.rs", "build.rs"]:
                    row["candidateSources"][path] = sha(tar.extractfile(member).read())
            if not row["notices"]:
                raise RuntimeError("missing published notice file for " + name)
            rows.append(row)
    upstream = []
    for name in ["LICENSE", "PATENTS"]:
        data = (args.text_cache / name).read_bytes()
        destination = notices / (sha(data) + ".txt")
        destination.write_bytes(data)
        upstream.append({"source": "Go/xtext selected source and derived Go15 White_Space ranges",
                         "path": name, "sha256": sha(data), "file": str(destination.relative_to(HERE))})
    feature_bytes = args.features.read_bytes()
    (HERE / "features.txt").write_bytes(feature_bytes)
    report = {"scope": "qualification graph only; no root/source/license policy selection or audit/deny pass",
              "host": "aarch64-apple-darwin", "rust": "1.99.1", "cargoLockSha256": sha((HERE / "Cargo.lock").read_bytes()),
              "packages": rows, "goTextNotices": upstream,
              "featuresSha256": sha(feature_bytes), "selectedRegistryPackages": len(selected),
              "lockedRegistryPackages": len(rows)}
    (HERE / "dependencies.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Recorded", len(rows), "locked packages;", len(selected), "selected registry packages")


if __name__ == "__main__":
    main()
