#!/usr/bin/env python3
"""Capture Task 4 queries using only manifest-verified frozen Go sources."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from frozen_go import frozen_module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    template = root / "scripts/storage-query-oracle.go.in"
    with frozen_module(root, "storage-query-") as (work, environment, manifest_bytes):
        driver = work / "driver/main.go"
        driver.parent.mkdir()
        driver.write_bytes(template.read_bytes())
        output = work / "queries.json"
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
