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
    for entry in json.loads(manifest.read_bytes())["files"]:
        path = entry["path"]
        if path.startswith("internal/") or path in ("go.mod", "go.sum"):
            if hashlib.sha256((root / path).read_bytes()).hexdigest() != entry["sha256"]:
                raise SystemExit(f"frozen source changed: {path}")
    tmp = root / "tmp"
    tmp.mkdir(exist_ok=True)
    template = root / "scripts/storage-query-oracle.go.in"
    with tempfile.TemporaryDirectory(prefix="storage-query-", dir=tmp) as work:
        driver = Path(work) / "main.go"
        driver.write_bytes(template.read_bytes())
        output = Path(work) / "queries.json"
        subprocess.run(
            ["go", "run", "-mod=readonly", str(driver),
             str(root / "preservation/corpus/databases"), str(output)],
            cwd=root, env={**os.environ, "GOWORK": "off"}, check=True, timeout=180,
        )
        captured = json.loads(output.read_bytes())
        captured["generatorSha256"] = hashlib.sha256(template.read_bytes()).hexdigest()
        captured["oracleSourceManifestSha256"] = hashlib.sha256(manifest.read_bytes()).hexdigest()
        args.output.write_text(json.dumps(captured, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
