#!/usr/bin/env python3
"""Observe pinned, synthetic PR39 blobs without checking out/executing its code.

Exit 0: comparisons ran, no measured mismatch (still partial qualification).
Exit 2: measured differences retained in report. Other failures are tooling errors.
The report never treats a decoded container as product acceptance.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import platform
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import uuid
import zlib

COMMIT = "e7c771b4218ef689dd6743fb485e4d9a52e23d43"
ORACLE = "73a93303a9a44cf0a92eaf525331561ed8c04460"
MANIFEST_SHA256 = "91ccf4eb2dc9d707d429af3568e8703848aa213b9a6a94727442e7bbce044966"
HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent


def spike_output(*arguments):
    """Select the nested pin for both execution and provenance, regardless of caller."""
    with (HERE / "rust-toolchain.toml").open("rb") as source:
        channel = tomllib.load(source)["toolchain"]["channel"]
    return subprocess.check_output(["rustup", "run", channel, *arguments], cwd=HERE)


def blob(path):
    return subprocess.check_output(["git", "show", f"{COMMIT}:preservation/corpus/{path}"], cwd=REPO)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def differences(expected, actual, path=""):
    """Keep object field presence, every array index, scalar type and value."""
    if type(expected) is not type(actual):
        return [{"path": path, "expected": expected, "actual": actual}]
    if isinstance(expected, dict):
        result = []
        for key in sorted(expected.keys() | actual.keys()):
            pointer = path + "/" + key.replace("~", "~0").replace("/", "~1")
            if key not in expected or key not in actual:
                result.append({"path": pointer, "expectedPresent": key in expected,
                               "actualPresent": key in actual, "expected": expected.get(key),
                               "actual": actual.get(key)})
            else:
                result.extend(differences(expected[key], actual[key], pointer))
        return result
    if isinstance(expected, list):
        result = []
        for index in range(max(len(expected), len(actual))):
            pointer = f"{path}/{index}"
            if index >= len(expected) or index >= len(actual):
                result.append({"path": pointer, "expectedPresent": index < len(expected),
                               "actualPresent": index < len(actual),
                               "expected": expected[index] if index < len(expected) else None,
                               "actual": actual[index] if index < len(actual) else None})
            else:
                result.extend(differences(expected[index], actual[index], pointer))
        return result
    return [] if expected == actual else [{"path": path, "expected": expected, "actual": actual}]


def payload_identity(entries):
    """Diagnostic evidence only: no suffix, structural or rejection classifier."""
    candidates = []
    for entry, data in entries:
        if entry["name"] == "uuid.bin" and len(data) == 16:
            candidates.append(str(uuid.UUID(bytes=data)))
        elif entry["name"] == "story.json":
            candidates.append(json.loads(data)["uuid"])
        elif "/" in entry["name"]:
            try:
                candidate = str(uuid.UUID(entry["name"].split("/", 1)[0]))
                if candidate not in candidates:
                    candidates.append(candidate)
            except ValueError:
                pass
    return candidates


def digest_entries(candidate):
    payloads, mismatches = [], []
    for entry in candidate.get("entries", []):
        data = bytes.fromhex(entry.pop("payloadHex"))
        entry["sha256"] = sha256(data)
        entry["observedCrc32"] = zlib.crc32(data)
        if entry.get("hasCrc", True) and entry["crc"] != entry["observedCrc32"]:
            mismatches.append({"name": entry["name"], "headerCrc": entry["crc"],
                               "observedCrc": entry["observedCrc32"]})
        payloads.append((entry, data))  # array order and duplicate names retained
    return payloads, mismatches


def generate_codecs(root, bsdtar):
    """Independent native producer; never represented as admitted Go corpus."""
    version = subprocess.check_output([bsdtar, "--version"], text=True).strip()
    directory = root / "independent-codecs"
    directory.mkdir()
    payloads = [("preceding.bin", b"synthetic preceding solid-block payload\n" * 12000),
                ("Été 日本語.bin", b"synthetic target\x00\xff\n" * 40)]
    expected = []
    input_tar = directory / "payloads.tar"
    # Archive-to-archive conversion avoids leaking host uid/ctime/birthtime into inputs.
    with tarfile.open(input_tar, "w", format=tarfile.PAX_FORMAT) as archive:
        for name, data in payloads:
            entry = tarfile.TarInfo(name)
            entry.size, entry.mtime, entry.mode = len(data), 946684800, 0o644
            entry.uid, entry.gid, entry.uname, entry.gname = 0, 0, "qualification", "qualification"
            archive.addfile(entry, io.BytesIO(data))
            expected.append({"name": name, "size": len(data), "sha256": sha256(data)})
    fixtures = []
    for method in ("copy", "lzma1", "lzma2", "bzip2", "deflate", "ppmd", "zstd"):
        file = f"independent-codecs/{method}.7z"
        result = subprocess.run([bsdtar, "-cf", str(root / file), "--format=7zip", "--options",
                                 "7zip:compression=" + method, "@" + str(input_tar)],
                                capture_output=True, text=True, check=False)
        fixture = {"file": file, "methodRequested": method, "producer": version,
                   "expectedEntries": expected}
        if result.returncode:
            fixture["producerError"] = result.stderr.strip()
        else:
            data = (root / file).read_bytes()
            fixture.update(bytes=len(data), sha256=sha256(data))
            fixture["producerListedNames"] = subprocess.check_output([bsdtar, "-tf", str(root / file)], text=True).splitlines()
        fixtures.append(fixture)
    # Mutate both preceding and target COPY payloads, retaining declared CRCs/header.
    original = (directory / "copy.7z").read_bytes()
    for index, (name, data) in enumerate(payloads):
        offset = original.find(data)
        assert offset >= 32, "COPY fixture must expose exact payload bytes"
        corrupt = bytearray(original)
        corrupt[offset] ^= 1
        file = f"independent-codecs/corrupt-{index}.7z"
        (root / file).write_bytes(corrupt)
        fixtures.append({"file": file, "methodRequested": "copy", "producer": version,
                         "mutatedEntry": name, "expectedContainerReadError": "ChecksumVerificationFailed",
                         "bytes": len(corrupt), "sha256": sha256(corrupt)})
    (root / "independent-codecs.json").write_text(json.dumps(fixtures))
    return fixtures


def compare(root, observed):
    report = {"status": "partial; corpus admission and product classification not complete",
              "fixtureCommit": COMMIT, "oracleCommit": ORACLE,
              "manifestSha256": MANIFEST_SHA256, "sqliteVersion": observed["sqliteVersion"],
              "databases": [], "archives": [], "measuredDifferenceCount": 0}
    for database in observed["databases"]:
        prefix = database["prefix"]
        if prefix is None:
            expected = json.loads((root / "libraries/schema018/state.json").read_bytes())
            after = expected
        else:
            fixture = json.loads((root / f"databases/prefix-{prefix:03}.json").read_bytes())
            expected, after = fixture["before"], fixture["afterGoOpen"]
        item = {"file": database["file"], "prefix": prefix,
                "beforeDifferences": differences(expected, database["before"]),
                "onlineBackupDifferences": differences(database["before"], database["backup"]),
                "afterSqlOnlyVsGoDifferences": differences(after, database["afterSqlOnly"])}
        report["measuredDifferenceCount"] += sum(len(item[k]) for k in (
            "beforeDifferences", "onlineBackupDifferences", "afterSqlOnlyVsGoDifferences"))
        report["databases"].append(item)
    oracle = json.loads((root / "inspection.json").read_bytes())
    assert [a["file"] for a in observed["archives"]] == [a["file"] for a in oracle]
    for expected, archive in zip(oracle, observed["archives"]):
        item = {"oracle": expected, "candidate": archive["observation"],
                "classificationComparison": "not implemented; decoder success is not acceptance",
                "payloadDifferences": []}
        payloads, item["payloadDifferences"] = digest_entries(item["candidate"])
        item["identityCandidates"] = payload_identity(payloads)
        if not expected["error"]:
            if "containerError" in item["candidate"]:
                item["payloadDifferences"].append({"expected": "readable accepted container",
                                                   "actual": item["candidate"]["containerError"]})
            if item["identityCandidates"] != [expected["uuid"]]:
                item["payloadDifferences"].append({"field": "uuid candidates", "expected": [expected["uuid"]],
                                                   "actual": item["identityCandidates"]})
            artwork = expected.get("artwork")
            if artwork:
                matches = [entry["sha256"] for entry, _ in payloads if entry["name"] == artwork["entry"]]
                if matches != [artwork["sha256"]]:
                    item["payloadDifferences"].append({"field": "artwork bytes", "expected": [artwork["sha256"]], "actual": matches})
        report["measuredDifferenceCount"] += len(item["payloadDifferences"])
        report["archives"].append(item)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write the complete evidence JSON; defaults to stdout")
    parser.add_argument("--bsdtar", help="explicit native bsdtar path for extra synthetic codec/CRC probes")
    args = parser.parse_args()
    manifest_bytes = blob("manifest.json")
    assert sha256(manifest_bytes) == MANIFEST_SHA256, "wrong pinned manifest"
    manifest = json.loads(manifest_bytes)
    assert manifest["oracle_commit"] == ORACLE
    with tempfile.TemporaryDirectory(prefix="librairii-2-corpus-") as temporary:
        root = Path(temporary)
        selected = []
        for item in manifest["files"]:
            name = item["path"]
            # Exclude all relocation/profile files and all generator programs.
            if not (name.startswith(("databases/", "archives/", "migrations/", "libraries/schema018/"))
                    or name == "inspection.json"):
                continue
            relative = PurePosixPath(name)
            assert not relative.is_absolute() and ".." not in relative.parts
            data = blob(name)
            assert len(data) == item["bytes"] and sha256(data) == item["sha256"], name
            path = root.joinpath(*relative.parts)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            selected.append(item)
        before = {i["path"]: sha256((root / i["path"]).read_bytes()) for i in selected}
        codecs = generate_codecs(root, args.bsdtar) if args.bsdtar else []
        result = spike_output("cargo", "run", "--locked", "--offline", "--manifest-path",
                              str(HERE / "Cargo.toml"), "--example", "corpus-probe", "--", str(root))
        assert before == {i["path"]: sha256((root / i["path"]).read_bytes()) for i in selected}, "fixture mutation"
        observed = json.loads(result)
        report = compare(root, observed)
        report["observer"] = {"session": "librairii-4", "date": "2026-10-20",
                              "host": platform.platform(),
                              "cargo": spike_output("cargo", "--version").decode().strip(),
                              "rustc": spike_output("rustc", "--version").decode().strip()}
        report["verifiedInputFiles"] = selected
        report["independentCodecs"] = []
        by_file = {c["file"]: c["observation"] for c in observed.get("independentCodecs", [])}
        for fixture in codecs:
            item = {"fixture": fixture, "differences": [], "scope": "independent synthetic decoder probe; no Go acceptance oracle"}
            if "producerError" not in fixture:
                assert sha256((root / fixture["file"]).read_bytes()) == fixture["sha256"]
                item["candidate"] = candidate = by_file[fixture["file"]]
                payloads, item["differences"] = digest_entries(candidate)
                if fixture.get("expectedContainerReadError"):
                    if fixture["expectedContainerReadError"] not in candidate.get("containerError", ""):
                        item["differences"].append({"expected": fixture["expectedContainerReadError"],
                                                    "actual": candidate.get("containerError", "decoded")})
                else:
                    entries = [{k: e[k] for k in ("name", "size", "sha256")} for e, _ in payloads]
                    item["differences"].extend(differences(fixture["expectedEntries"], entries))
            report["measuredDifferenceCount"] += len(item["differences"])
            report["independentCodecs"].append(item)
    output = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(output)
    else:
        print(output, end="")
    print(f"Partial observations: {len(report['databases'])} databases, {len(report['archives'])} containers, "
          f"{report['measuredDifferenceCount']} measured differences. Classification/admission remain open.",
          file=sys.stderr)
    return 2 if report["measuredDifferenceCount"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
