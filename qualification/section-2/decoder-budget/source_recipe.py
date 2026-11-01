#!/usr/bin/env python3
"""Authenticate/reproduce the qualification derivative; never read expanded caches."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def authenticate(archive, manifest):
    data = archive.read_bytes()
    require(len(data) == manifest["crate"]["bytes"] and digest(data) == manifest["crate"]["sha256"],
            "crate archive identity mismatch")
    expected = {row["path"]: row for row in manifest["members"]}
    require(len(expected) == len(manifest["members"]), "duplicate manifest member")
    captured = {}
    prefix = manifest["crate"]["name"] + "-" + manifest["crate"]["version"] + "/"
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            require(member.isfile() and member.name.startswith(prefix), "nonregular/wrong-prefix crate member")
            relative = member.name[len(prefix):]
            parts = PurePosixPath(relative)
            require(relative and not parts.is_absolute() and ".." not in parts.parts
                    and "\\" not in relative and str(parts) == relative, "unsafe crate member")
            require(relative in expected and relative not in captured, "extra/duplicate crate member")
            content = source.extractfile(member).read()
            row = expected[relative]
            require(len(content) == row["bytes"] and digest(content) == row["sha256"], "crate member hash mismatch")
            if row["selected"]:
                require(content.decode().splitlines()[:12] == row["first12Lines"], "source header mismatch")
            captured[relative] = content
    require(set(captured) == set(expected), "missing crate members")
    return {path: content for path, content in captured.items() if expected[path]["selected"]}


def verify_tree(root, expected):
    require(root.is_dir() and not root.is_symlink(), "missing/nonregular source root")
    actual = {}
    for path in root.rglob("*"):
        require(not path.is_symlink(), "source tree symlink")
        if path.is_dir():
            continue
        require(path.is_file(), "nonregular source tree member")
        actual[str(path.relative_to(root))] = path.read_bytes()
    rows = {row["path"]: row for row in expected}
    require(len(rows) == len(expected), "duplicate derivative manifest member")
    require(set(actual) == set(rows), "source tree extra/missing member")
    for path, data in actual.items():
        require(len(data) == rows[path]["bytes"] and digest(data) == rows[path]["sha256"], "derivative source hash mismatch")


def reproduce(archive, output):
    manifest = json.loads((HERE / "upstream-source.json").read_text())
    originals = authenticate(archive, manifest)
    patch = HERE / "count-budget.patch"
    require(digest(patch.read_bytes()) == manifest["patchSha256"], "derivative patch hash mismatch")
    require(not output.exists() and not output.is_symlink(), "output must be a new owned directory")
    output.mkdir(parents=True)
    for relative, data in originals.items():
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    subprocess.run(["patch", "--batch", "--fuzz=0", "-p1", "-i", str(patch)],
                   cwd=output, check=True, capture_output=True, timeout=10)
    verify_tree(output, manifest["derivedFiles"])
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="librairii-4-crate-source-") as temporary:
        manifest = reproduce(args.archive.resolve(), Path(temporary) / "reproduced")
        verify_tree(HERE / "upstream/sevenz-rust2-0.24.1", manifest["derivedFiles"])
    print(json.dumps({"authenticatedRegularMembers": len(manifest["members"]),
                      "derivedFiles": len(manifest["derivedFiles"]), "status": "exact authenticated derivative"}))


if __name__ == "__main__":
    main()
