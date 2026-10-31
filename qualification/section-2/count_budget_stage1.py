#!/usr/bin/env python3
"""QUAL2.10-392-COUNT-BUDGET: exactly two retained negatives plus one control.

No uncontained decoder execution, no original32-case dispatch, no product policy.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
import zlib

import decoder_containment as boundary
from synthetic_7z import archive, signature, uint64

HERE = Path(__file__).resolve().parent
IDENTITIES = {
    "files-count-allocation": (2097184, "1de99665b09c5a6209ad222a6be49825ef56cfa319246131a53b4c4f9a6b13f0"),
    "coders-count-allocation-larger": (8388640, "5acc6aa505a5f0aab9c6ed6437ce2ce86b999491ff5ba1ac6ef9769d5daf1ccb"),
    "solid-ordered": (214, "a7c1855fcb49e33fe9827a60e87454c8ed8942cb7f897350fe1cac32888e4988"),
}
GUARDS = {"files": 10000, "codersPerBlock": 64, "perVectorBytes": 8 << 20}


def fixtures(directory):
    # Exact retained recipes, without generating any other adversarial category.
    files_header = b"\x01\x05" + uint64(2000000) + b"\x00\x00"
    coders_header = b"\x01\x04\x07\x0b\x01\x00" + uint64(8000000)
    entries = [("first.bin", b"first solid payload\n" * 200),
               ("second.bin", b"second solid payload\x00\xff\n" * 200)]
    inputs = [signature(b"", files_header + bytes((2 << 20) - len(files_header))),
              signature(b"", coders_header + bytes((8 << 20) - len(coders_header))),
              archive(entries, "lzma2")]
    result = []
    for (name, identity), data in zip(IDENTITIES.items(), inputs, strict=True):
        boundary.require((len(data), hashlib.sha256(data).hexdigest()) == identity,
                         "retained fixture identity mismatch; no decoder may run")
        file = name + ".7z"
        (directory / file).write_bytes(data)
        row = {"name": name, "file": file, "bytes": len(data), "sha256": identity[1],
               "producer": "original retained recipe; synthetic_7z.py; no decoder or upstream fixture used"}
        if name == "solid-ordered":
            row["expectedDecoded"] = [{"name": n, "bytes": len(p), "crc32": zlib.crc32(p),
                                       "payloadSha256": hashlib.sha256(p).hexdigest()} for n, p in entries]
        else:
            row["expectedRefusal"] = {"field": "num files" if name.startswith("files") else "num coders",
                                      "count": 2000000 if name.startswith("files") else 8000000,
                                      "maxCount": 10000 if name.startswith("files") else 64}
        result.append(row)
    return result


def evaluate(fixture, observed):
    # OS kill, timeout, missing measurements or malformed input errors never pass.
    actual = boundary.verdict(observed)
    classification = boundary.classification(observed, actual)
    failures = []
    if observed["exitCode"] != 0 or observed["wallWatchdog"] or observed["oomKilled"]:
        failures.append("process did not complete normally; harness termination is not decoder refusal")
    events = observed["memoryEventsAfter"]
    if not events or events.get("oom", 0) or events.get("oom_kill", 0):
        failures.append("missing/failed independent memory.events evidence")
    if observed["cgroupMemoryPeakAfter"] is None:
        failures.append("missing independent cgroup memory peak")
    records = observed["records"]
    boot = records[0] if records else {}
    if (boot.get("memoryMax", "").strip() != str(boundary.MEMORY)
            or boot.get("swapMax", "").strip() != "0"
            or not re.search(rf"Max address space\s+{boundary.ADDRESS}\s+{boundary.ADDRESS}", boot.get("limits", ""))
            or not re.search(r"Max cpu time\s+2\s+2", boot.get("limits", ""))):
        failures.append("case effective kernel boundary evidence mismatch")
    markers = [r for r in records if r.get("stage") == "before_constructor"]
    if len(markers) != 1 or markers[0].get("experimentalCountGuards") != GUARDS:
        failures.append("missing/incorrect constructor guard marker")
    terminal = records[-1] if records else {}
    if "expectedRefusal" in fixture:
        details = terminal.get("details", {})
        expected = dict(fixture["expectedRefusal"], stage="header_before_target_vector_allocation",
                        maxVectorBytes=GUARDS["perVectorBytes"], reason="count_ceiling", callbackCount=0)
        if actual != "qualification_count_budget_refused" or any(details.get(k) != v for k, v in expected.items()):
            failures.append("missing explicit typed targeted count-budget refusal")
        else:
            classification = "experimental_target_vector_count_budget_refusal"
        if (not isinstance(details.get("elementBytes"), int) or details["elementBytes"] <= 0
                or details.get("requestedBytes") != details["elementBytes"] * expected["count"]):
            failures.append("missing/incorrect checked requested-vector byte calculation")
        if observed["callbackCount"] != 0:
            failures.append("callback occurred before refusal")
    else:
        decoded = terminal.get("details", {}).get("decoded", [])
        expected = [{k: row[k] for k in ("name", "bytes", "crc32")} for row in fixture["expectedDecoded"]]
        projected = [{"name": row["name"], "bytes": row["bytes"], "crc32": row["observedCrc32"]} for row in decoded]
        if actual != "decoded" or observed["callbackCount"] != 2 or projected != expected:
            failures.append("retained benign ordered payload control differs")
        if any(not r["hasCrc"] or r["headerCrc"] != r["observedCrc32"] for r in decoded):
            failures.append("retained benign CRC mismatch")
    return {"fixture": fixture, "actual": actual, "classification": classification,
            "failures": failures, "observation": observed}


def execute(binary, directory, report):
    boundary.controls(binary, directory, report["controls"])
    # The ONLY path to constructor execution follows all actual passing controls.
    for fixture in fixtures(directory):
        observed = boundary.contained(binary, directory, "guarded", fixture["file"])
        report["cases"].append(evaluate(fixture, observed))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    boundary.require(not args.output.exists() and not args.output.is_symlink(),
                     "observation output must be new; historical reports must not be overwritten")
    binary = args.binary.resolve()
    data = binary.read_bytes()
    boundary.require(len(data) >= 64 and data[:6] == b"\x7fELF\x02\x01" and data[18:20] == b"\xb7\x00",
                     "expected LinuxARM64 little-endian ELF; no fallback")
    boundary.require(os.environ.get("AO_SESSION_ID") == "librairii-4", "owned AO session required")
    source_paths = [Path(__file__), HERE / "decoder_containment.py", HERE / "synthetic_7z.py",
                    HERE / "decoder-budget/Cargo.toml", HERE / "decoder-budget/Cargo.lock",
                    HERE / "decoder-budget/src/main.rs", HERE / "decoder-budget/source_recipe.py",
                    HERE / "decoder-budget/count-budget.patch", HERE / "decoder-budget/upstream-source.json"]
    report = {"release": "QUAL2.10-392-COUNT-BUDGET", "scope": "qualification-only LinuxARM; no product/desktop/resource qualification",
              "startingHead": "39267cffd281223cb9295f276cf6f7c041fcc701", "admittedBase": "4383373272ce1d3095199caf37dde7e5d0598213",
              "image": boundary.IMAGE, "binarySha256": hashlib.sha256(data).hexdigest(),
              "limits": {"cgroupMemoryBytes": boundary.MEMORY, "swapBytes": 0, "addressSpaceBytes": boundary.ADDRESS,
                         "cpuSeconds": 2, "wallSeconds": 5}, "experimentalCountGuards": GUARDS,
              "sourceSha256": {str(p.relative_to(HERE)): hashlib.sha256(p.read_bytes()).hexdigest() for p in source_paths},
              "observer": {"session": os.environ["AO_SESSION_ID"], "python": sys.version, "host": platform.platform(),
                           "dockerServer": json.loads(boundary.docker("version", "--format", "{{json .Server}}")),
                           "kernel": boundary.docker("info", "--format", "{{.KernelVersion}}")},
              "controls": [], "cases": [], "status": "unqualified"}
    try:
        with tempfile.TemporaryDirectory(prefix="librairii-4-count-stage1-") as temporary:
            execute(binary, Path(temporary), report)
        report["status"] = "targeted observations only; all wider gates open"
    except (RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
        report["toolingError"] = str(error)
        report["status"] = "tooling/control failure; not a decoder verdict"
        raise
    finally:
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(report["controls"]), "cases": len(report["cases"]),
                      "failures": sum(len(c["failures"]) for c in report["cases"]), "status": report["status"]}))
    return 2 if any(c["failures"] for c in report["cases"]) else 0


if __name__ == "__main__":
    raise SystemExit(main())
