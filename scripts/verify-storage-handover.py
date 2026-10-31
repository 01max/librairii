#!/usr/bin/env python3
"""Run the finite Go/Rust storage rehearsal, preserving source/tool identities."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from frozen_go import frozen_module


def main():
    root = Path(__file__).resolve().parent.parent
    template = root / "scripts/storage-legacy-probe.go.in"
    with frozen_module(root, "storage-handover-") as (work, environment, manifest):
        driver = work / "driver/main.go"
        driver.parent.mkdir()
        driver.write_bytes(template.read_bytes())
        helper = work / "legacy-probe"
        subprocess.run(["go", "build", "-mod=readonly", "-o", str(helper), "./driver"],
                       cwd=work, env=environment, check=True, timeout=180)
        command = ["cargo", "run", "--locked", "-p", "librairii-storage",
                   "--example", "storage_handover"]
        if os.environ.get("RUST_TARGET"):
            command += ["--target", os.environ["RUST_TARGET"]]
        command += ["--", str(helper)]
        result = subprocess.run(command, cwd=root, capture_output=True,
                                text=True, timeout=120)
        if result.returncode:
            raise SystemExit(f"storage rehearsal failed ({result.returncode})\n{result.stdout}\n{result.stderr}")
        report = json.loads(result.stdout)
        report.update(
            source=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
            go=subprocess.check_output(["go", "version"], env=environment, text=True).strip(),
            rust=subprocess.check_output(["rustc", "-vV"], cwd=root, text=True),
            oracleCommit="73a93303a9a44cf0a92eaf525331561ed8c04460",
            oracleSourceManifestSha256=hashlib.sha256(manifest).hexdigest(),
            driverSha256=hashlib.sha256(template.read_bytes()).hexdigest(),
            helperSha256=hashlib.sha256(helper.read_bytes()).hexdigest(),
        )
        output = root / "target/quality/storage-handover.json"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
