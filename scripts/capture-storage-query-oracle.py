#!/usr/bin/env python3
"""Capture Task 4 queries using only manifest-verified frozen Go sources."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    manifest = root / "preservation/oracle-source.json"
    tmp = root / "tmp"
    tmp.mkdir(exist_ok=True)
    template = root / "scripts/storage-query-oracle.go.in"
    with tempfile.TemporaryDirectory(prefix="storage-query-", dir=tmp) as work:
        work = Path(work)
        manifest_bytes = manifest.read_bytes()
        # Materialize only verified manifest members, from the SAME checked
        # buffers. Extra/untracked checkout sources can never enter this build.
        for entry in json.loads(manifest_bytes)["files"]:
            path = entry["path"]
            if path.startswith("internal/") or path in ("go.mod", "go.sum"):
                body = (root / path).read_bytes()
                if hashlib.sha256(body).hexdigest() != entry["sha256"]:
                    raise SystemExit(f"frozen source changed: {path}")
                target = work / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(body)
        driver = work / "driver/main.go"
        driver.parent.mkdir()
        driver.write_bytes(template.read_bytes())
        output = work / "queries.json"
        environment = {key: value for key, value in os.environ.items()
                       if not key.startswith("GO")}
        environment.update(GOWORK="off", GOENV="off", GOFLAGS="", GOTOOLCHAIN="local")
        subprocess.run(
            ["go", "run", "-mod=readonly", "./driver",
             str(root / "preservation/corpus/databases"), str(output)],
            cwd=work, env=environment, check=True, timeout=180,
        )
        captured = json.loads(output.read_bytes())
        captured["generatorSha256"] = hashlib.sha256(template.read_bytes()).hexdigest()
        captured["oracleSourceManifestSha256"] = hashlib.sha256(manifest_bytes).hexdigest()
        args.output.write_text(json.dumps(captured, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
