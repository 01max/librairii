#!/usr/bin/env python3
"""Rebuild the scoped reference in disposable independent, authenticated inputs.

This qualification recipe never runs Go in the frozen oracle, changes the
preservation corpus, downloads a module, or supplies a production runtime.
"""
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import zipfile


ORACLE = "73a93303a9a44cf0a92eaf525331561ed8c04460"
ORACLE_TREE = "e797e9d59926d83b73fb7c02e5c3611073e89a16"
ZIP_SHA = "b75131f628f6e2e4892fd638d33127e952a5027775e0162b94ba069cb4e13863"
# Future lookups use the admitted superset; existing reference provenance is historical.
HISTORICAL_INDEX_SHA = "10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99"
INDEX_SHA = "cd8fcdac9ccdd46f1f06294f27bc07e9f8ed40f79d16cdab4d39c7c0cd837d78"
GO_ROOT = Path("/opt/homebrew/Cellar/go/1.26.5/libexec")
ZIP = Path("/Users/maxime/go/pkg/mod/cache/download/golang.org/x/text/@v/v0.43.0.zip")
BUNDLE = "go-1.26.5-xtext-0.43.0-unicode15-cldr32"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError("qualification inputs cannot contain symlinks")
        if path.is_file():
            result[path.relative_to(root).as_posix()] = sha(path.read_bytes())
    return result


def command(args, cwd, env=None, timeout=120):
    return subprocess.run(args, cwd=cwd, env=env, timeout=timeout,
                          check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE).stdout


def qualify(template_name="normalization_oracle.go.in", arguments=(), runner=None, copies=()):
    directory = Path(__file__).resolve().parent
    repository = directory.parents[4]
    index_bytes = (repository / ".config/upstream-sources" / BUNDLE / "source-index.json").read_bytes()
    if sha(index_bytes) != INDEX_SHA:
        raise ValueError("changed authenticated source index")
    index = json.loads(index_bytes)
    sources = [f for package in index["packages"] for f in package["files"]]
    sources += index["supplementalSourceFiles"]
    source_by_name = {f["path"]: f for f in sources}
    zip_bytes = ZIP.read_bytes()
    if sha(zip_bytes) != ZIP_SHA:
        raise ValueError("changed authenticated xtext ZIP")
    tree = command(["git", "rev-parse", ORACLE + "^{tree}"], repository).decode().strip()
    if tree != ORACLE_TREE:
        raise ValueError("different frozen Go tree")
    archive_bytes = command(["git", "archive", "--format=tar", ORACLE], repository)
    go_tool = GO_ROOT / "bin/go"
    go_tool_sha = sha(go_tool.read_bytes())
    original_go = {}
    for source in sources:
        if source["path"].startswith("Go1.26.5/"):
            path = GO_ROOT / source["path"].removeprefix("Go1.26.5/")
            content = path.read_bytes()
            if sha(content) != source["sha256"]:
                raise ValueError("changed selected Go source")
            original_go[str(path)] = sha(content)

    with tempfile.TemporaryDirectory(prefix="librairii-normalization-proof-") as scratch:
        temporary = Path(scratch)
        checkout = temporary / "frozen"
        checkout.mkdir(mode=0o700)
        with tarfile.open(fileobj=io.BytesIO(archive_bytes)) as archive:
            for member in archive.getmembers():
                name = PurePosixPath(member.name)
                if name.is_absolute() or ".." in name.parts or not (member.isfile() or member.isdir()):
                    raise ValueError("nonregular frozen archive input")
            archive.extractall(checkout, filter="data")
        frozen_before = inventory(checkout)
        # A qualification may compile an EXACT original Go file alone under a
        # new internal package path, avoiding unrelated SQL-driver linkage. Its
        # bytes/package/algorithms are unchanged; the original remains in place.
        copied = {}
        for original, destination in copies:
            if original not in frozen_before or destination in frozen_before:
                raise ValueError("invalid isolated source copy")
            relative = PurePosixPath(destination)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("source copy escapes independent checkout")
            output = checkout / destination
            output.parent.mkdir(parents=True, exist_ok=True)
            content = (checkout / original).read_bytes()
            output.write_bytes(content)
            copied[destination] = {"original": original, "sha256": sha(content)}

        module = temporary / "xtext"
        module.mkdir(mode=0o700)
        with zipfile.ZipFile(io.BytesIO(zip_bytes)) as archive:
            members = archive.infolist()
            if len(members) != 487 or len({m.filename for m in members}) != 487:
                raise ValueError("changed authenticated ZIP member closure")
            prefix = "golang.org/x/text@v0.43.0/"
            for member in members:
                if not member.filename.startswith(prefix):
                    raise ValueError("foreign authenticated ZIP member")
                name = member.filename.removeprefix(prefix)
                relative = PurePosixPath(name)
                if (not name or relative.is_absolute() or ".." in relative.parts
                        or member.is_dir() or (member.external_attr >> 16) & 0o170000 == 0o120000):
                    raise ValueError("nonregular authenticated ZIP member")
                content = archive.read(member)
                selected = source_by_name.get(member.filename)
                if selected is not None and (len(content), sha(content)) != (selected["bytes"], selected["sha256"]):
                    raise ValueError("selected source differs from authenticated index")
                output = module / name
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(content)
        xtext_before = inventory(module)

        probe = checkout / "cmd/native-normalization-proof"
        probe.mkdir(parents=True)
        template = directory / template_name
        (probe / "main.go").write_bytes(template.read_bytes())
        modfile = temporary / "proof.mod"
        modfile.write_text("module github.com/01max/librairii\n\ngo 1.26.0\n"
                           "require golang.org/x/text v0.43.0\n"
                           "replace golang.org/x/text => " + str(module) + "\n", encoding="utf-8")
        env = dict(os.environ, GOTOOLCHAIN="local", GOWORK="off", GOPROXY="off", GOSUMDB="off",
                   CGO_ENABLED="0", GOCACHE=str(temporary / "build-cache"),
                   GOMODCACHE=str(temporary / "module-cache"), GOPATH=str(temporary / "go-path"),
                   GOTMPDIR=str(temporary / "go-tmp"))
        Path(env["GOTMPDIR"]).mkdir()
        binary = temporary / "normalization-proof"
        command([str(go_tool), "build", "-mod=mod", "-modfile=" + str(modfile),
                 "-o", str(binary), "./cmd/native-normalization-proof"], checkout, env)
        if runner is None:
            result = json.loads(command([str(binary), *arguments], checkout, env, timeout=120))
        else:
            result = runner({"temporary": temporary, "checkout": checkout, "module": module,
                             "binary": binary, "environment": env, "modfile": modfile,
                             "go_tool": go_tool, "sources": source_by_name,
                             "arguments": arguments, "module_inventory": xtext_before})
        frozen_after = inventory(checkout)
        probe_name = "cmd/native-normalization-proof/main.go"
        del frozen_after[probe_name]
        for destination, record in copied.items():
            if frozen_after.pop(destination) != record["sha256"]:
                raise ValueError("qualification changed an exact source copy")
        if frozen_after != frozen_before or inventory(module) != xtext_before:
            raise ValueError("qualification changed an original independent input")
        if sha(ZIP.read_bytes()) != ZIP_SHA or sha(go_tool.read_bytes()) != go_tool_sha:
            raise ValueError("qualification source/tool changed")
        if any(sha(Path(path).read_bytes()) != value for path, value in original_go.items()):
            raise ValueError("qualification changed a selected Go source")
        result["provenance"] = {
            "oracle": ORACLE, "tree": tree, "oracleArchiveSha256": sha(archive_bytes),
            "originalFiles": len(frozen_before), "unchangedOriginalFiles": len(frozen_after),
            "authenticatedZipSha256": ZIP_SHA, "unchangedZipMembers": len(xtext_before),
            "sourceIndexSha256": INDEX_SHA, "goToolSha256": go_tool_sha,
            "templateSha256": sha(template.read_bytes()),
            **({"exactSourceCopies": copied} if copied else {}),
            "scope": "disposable independent archive + external modfile; offline pinned Go1.26.5, no original input writes",
        }
        return result


def main():
    result = qualify()
    # Preserve the original source copyright lines on this generated
    # reference. Its source-manifest record supplies the exact identities.
    result["sourceCopyright"] = ["// Copyright 2011 The Go Authors. All rights reserved.",
                                 "// Copyright 2016 The Go Authors. All rights reserved.",
                                 "// Copyright 2009 The Go Authors. All rights reserved."]
    output = Path(__file__).resolve().parent / "normalization-reference.json"
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"reference": str(output), "sha256": sha(output.read_bytes()),
                      "scalarCount": result["scalarCount"], "sequenceCount": result["sequenceCount"]}))


if __name__ == "__main__":
    main()
