#!/usr/bin/env python3
"""Run a separate isolated observer after acceptance; never approve its gates."""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=["plan", "phase"])
    args = parser.parse_args()
    destination = Path("target/query-" + args.kind + "-experiment")
    destination.mkdir()  # An existing run is evidence, never overwrite it.
    shutil.copyfile("target/quality/native-scale/manifest.json", destination / "backend-manifest.json")
    manifest = json.loads((destination / "backend-manifest.json").read_bytes())
    binary = Path("target/quality/native-scale/scale-backend")

    def identity():
        source = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
        if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"]):
            raise ValueError("diagnostic requires clean source")
        raw = binary.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        if (source != manifest["source_commit"] or digest != manifest["executable"]["sha256"]
                or len(raw) != manifest["executable"]["bytes"]):
            raise ValueError("diagnostic executable/source differs from retained backend")
        return {"source_commit": source, "target": manifest["target"],
                "executable_sha256": digest, "executable_bytes": len(raw)}

    before = identity()
    (destination / "identity-before.json").write_text(json.dumps(before, indent=2) + "\n")
    try:
        with (destination / "report.json").open("x") as report, (destination / "stderr.log").open("x") as log:
            option = "--query-diagnostics" if args.kind == "plan" else "--query-phases"
            subprocess.run([str(binary), option], stdout=report, stderr=log, check=True, timeout=180)
    finally:
        after = identity()
        (destination / "identity-after.json").write_text(json.dumps(after, indent=2) + "\n")
        if before != after:
            raise ValueError("diagnostic identity changed during execution")


if __name__ == "__main__":
    main()
