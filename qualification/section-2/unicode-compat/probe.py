#!/usr/bin/env python3
"""Bounded synthetic Unicode experiment. Never build/run in the frozen oracle."""
import argparse
import base64
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import tomllib
import zipfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ORACLE = "73a93303a9a44cf0a92eaf525331561ed8c04460"
TREE = "e797e9d59926d83b73fb7c02e5c3611073e89a16"
CORPUS = "1cd22443cae3b548bb7f9b3fb2435dcd73c3c65c"
MANIFEST = "a59f2194cf5c1ad317485a8714e703c12fe161bfad0190a67ba5816ea7f46e53"
FOLD = "547a5aeae46f102d6a5310dcc5125988c6233d45125e96ffd712847a9c07b5de"
TEXT_SUM = "h1:1QivrlhwAsnMOcqgOdl9my7s9OyZl8O5/UGUTJVXoWc="
MAX_CASES = 50000


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def command(argv, cwd, env=None, timeout=120):
    result = subprocess.run(list(map(str, argv)), cwd=cwd, env=env, capture_output=True,
                            timeout=timeout, check=False)
    require(result.returncode == 0,
            f"tooling failure {argv[0]} (not compatibility result): {result.stderr.decode(errors='replace')}")
    return result.stdout


def identities(root, files):
    require(len(files) == 436, "oracle source manifest must contain 436 files")
    for row in files:
        path = root / row["path"]
        require(path.resolve().is_relative_to(root.resolve()) and not path.is_symlink(),
                "unsafe oracle source path")
        data = path.read_bytes()
        require(len(data) == row["bytes"] and digest(data) == row["sha256"],
                "oracle source identity mismatch: " + row["path"])
    return len(files)


def source_files():
    manifest = json.loads((REPO / "preservation/oracle-source.json").read_bytes())
    return next(v for v in manifest.values() if isinstance(v, list) and v
                and isinstance(v[0], dict) and "path" in v[0])


def check_oracle(root, files):
    require(command(["git", "rev-parse", "HEAD"], root).decode().strip() == ORACLE,
            "wrong immutable oracle commit")
    require(command(["git", "rev-parse", "HEAD^{tree}"], root).decode().strip() == TREE,
            "wrong immutable oracle tree")
    require(not command(["git", "status", "--porcelain"], root).strip(), "oracle is not clean")
    return identities(root, files)


def verify_text(cache):
    """Go HashZip/Hash1 format, independently recomputed without running Go."""
    module_root = cache.parents[2]
    archive = module_root / "cache/download/golang.org/x/text/@v/v0.43.0.zip"
    summary = hashlib.sha256()
    with zipfile.ZipFile(archive) as z:
        names = sorted(n for n in z.namelist() if not n.endswith("/"))
        for name in names:
            require("\n" not in name, "invalid module ZIP name")
            data = z.read(name)
            summary.update(f"{digest(data)}  {name}\n".encode())
            relative = name.removeprefix("golang.org/x/text@v0.43.0/")
            require(relative != name and (cache / relative).read_bytes() == data,
                    "module source cache differs from ZIP")
    actual = "h1:" + base64.b64encode(summary.digest()).decode()
    require(actual == TEXT_SUM, "x/text module checksum differs from frozen go.sum")
    return {"module": "golang.org/x/text v0.43.0", "h1": actual,
            "zipSha256": digest(archive.read_bytes()), "verifiedMembers": len(names)}


def corpus_inputs(directory):
    raw = command(["git", "show", f"{CORPUS}:preservation/corpus/manifest.json"], REPO)
    require(digest(raw) == MANIFEST, "wrong admitted corpus manifest")
    manifest = json.loads(raw)
    require(len(manifest["files"]) == 146, "wrong corpus file count")
    for row in manifest["files"]:
        path = REPO / "preservation/corpus" / row["path"]
        data = path.read_bytes()
        require(len(data) == row["bytes"] and digest(data) == row["sha256"], "corpus bytes changed")
    inputs = {}
    for name in ["unicode.json", "saved-queries.json"]:
        data = command(["git", "show", f"{CORPUS}:preservation/corpus/{name}"], REPO)
        (directory / name).write_bytes(data)
        inputs[name] = {"sha256": digest(data), "bytes": len(data)}
    return inputs


def admitted_cases(directory):
    rows = []
    for i, row in enumerate(json.loads((directory / "unicode.json").read_bytes())):
        rows.append({"name": f"admitted-unicode/{i}", "input": row["input"],
                     "expectedNormalized": row["normalized"]})

    def names(value, path):
        if isinstance(value, dict):
            for key, item in value.items():
                if key == "name" and isinstance(item, str):
                    rows.append({"name": path + "/name", "input": item})
                else:
                    names(item, path + "/" + key)
        elif isinstance(value, list):
            for i, item in enumerate(value):
                names(item, path + "/" + str(i))

    for row in json.loads((directory / "saved-queries.json").read_bytes()):
        payload = row["input"]["payload"]
        try:
            value = json.loads(payload)
        except json.JSONDecodeError:
            continue
        names(value, "admitted-saved/" + row["input"]["id"])
    return rows


def bounded_cases(rows):
    require(0 < len(rows) <= MAX_CASES, "synthetic sequence count outside bound")
    seen = set()
    for row in rows:
        require(set(row) <= {"name", "input", "expectedNormalized"}
                and isinstance(row.get("name"), str) and isinstance(row.get("input"), str),
                "invalid synthetic sequence identity")
        require(row["name"] not in seen, "duplicate synthetic sequence identity")
        seen.add(row["name"])
        require(len(row["input"].encode()) <= 4096, "synthetic sequence exceeds text bound")
    return rows


def paired(go_binary, rust_binary, fold, clone, environment, cases=None):
    args = [go_binary, "-mode", "sequences" if cases else "scalars"]
    if cases:
        args += ["-cases", cases]
    start = time.monotonic()
    with tempfile.TemporaryFile() as errors:
        oracle = subprocess.Popen(list(map(str, args)), cwd=clone, env=environment,
                                  stdout=subprocess.PIPE, stderr=errors)
        candidate = None
        try:
            candidate = subprocess.Popen([str(rust_binary), str(fold), str(cases) if cases else "scalars"],
                                         cwd=HERE, env=environment, stdin=oracle.stdout,
                                         stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            oracle.stdout.close()
            output, stderr = candidate.communicate(timeout=120)
            code = oracle.wait(timeout=10)
            errors.seek(0)
            require(code == 0, "Go oracle stream failed: " + errors.read().decode(errors="replace"))
            require(candidate.returncode in (0, 2), "candidate protocol/tooling failure: " + stderr.decode(errors="replace"))
            require(len(output) <= 16 << 20, "candidate evidence exceeds bound")
            report = json.loads(output)
            require(report["allDifferencesRecorded"], "difference recording budget exceeded; incomplete")
            require((candidate.returncode == 0) == (report["status"] == "PASS"), "inconsistent candidate exit/status")
            report.update({"goExit": code, "candidateExit": candidate.returncode,
                           "elapsedSeconds": round(time.monotonic() - start, 4)})
            return report
        finally:
            for process in [candidate, oracle]:
                if process and process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)


def json_stream(data):
    decoder = json.JSONDecoder()
    remaining = data.decode()
    while remaining.strip():
        value, end = decoder.raw_decode(remaining.lstrip())
        yield value
        remaining = remaining.lstrip()[end:]


def language_closure(go, clone, environment, module_cache):
    data = command([go, "list", "-mod=readonly", "-deps", "-json", "golang.org/x/text/language"], clone, environment)
    packages = []
    for package in json_stream(data):
        if package.get("Standard"):
            continue
        require(package["ImportPath"].startswith("golang.org/x/text/"), "unexpected language source dependency")
        files = []
        for name in package.get("GoFiles", []) + package.get("EmbedFiles", []):
            path = Path(package["Dir"]) / name
            files.append({"path": str(path.relative_to(module_cache)), "sha256": digest(path.read_bytes()),
                          "bytes": path.stat().st_size})
        packages.append({"package": package["ImportPath"], "imports": package.get("Imports", []), "files": files})
    return {"scope": "conservative compilation dependency/data closure of language package; source only",
            "packages": packages, "languageExecution": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, required=True)
    parser.add_argument("--text-cache", type=Path, required=True)
    parser.add_argument("--go", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--suite", choices=["baseline", "mixed"], default="baseline")
    args = parser.parse_args()
    oracle_root = args.oracle.resolve()
    require(not args.output.resolve().is_relative_to(oracle_root), "output cannot touch immutable oracle")
    files = source_files()
    report = {"scope": "qualification-only Unicode candidate experiment; not product or toolkit selection",
              "oracle": ORACLE, "tree": TREE, "corpus": CORPUS, "manifestSha256": MANIFEST,
              "host": {"platform": platform.platform(), "machine": platform.machine()},
              "status": "tooling incomplete"}
    report["suite"] = args.suite
    try:
        report["immutableOracleVerifiedBefore"] = check_oracle(oracle_root, files)
        report["textSource"] = verify_text(args.text_cache.resolve())
        environment = dict(os.environ)
        build_variables = ["RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
                           "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS", "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER"]
        report["inheritedBuildVariablePresence"] = {k: bool(environment.get(k)) for k in build_variables}
        require(not any(report["inheritedBuildVariablePresence"].values()), "unexpected inherited target/flags/runner")
        for key in ["RUSTUP_TOOLCHAIN", "RUSTUP_HOME"]:
            environment.pop(key, None)
        rustup = shutil.which("rustup")
        require(rustup is not None, "rustup proxy required")
        rust = [rustup, "run", "1.99.1"]
        report["rustc"] = command(rust + ["rustc", "-vV"], HERE, environment).decode()
        require("release: 1.99.1\n" in report["rustc"], "wrong stable Rust release")
        host = next(line.removeprefix("host: ") for line in report["rustc"].splitlines() if line.startswith("host: "))
        require(host == "aarch64-apple-darwin", "this observation requires the documented actual ARM Mac host")
        report["target"] = host
        report["cargo"] = command(rust + ["cargo", "--version"], HERE, environment).decode().strip()
        report["globalDefaultBefore"] = command([rustup, "default"], HERE, environment).decode().strip()
        with tempfile.TemporaryDirectory(prefix="librairii-4-unicode-") as temporary:
            owned = Path(temporary)
            clone = owned / "oracle-clone"
            command(["git", "clone", "--no-local", "--no-hardlinks", oracle_root, clone], owned)
            report["disposableCloneVerifiedBefore"] = check_oracle(clone, files)
            require(not (clone / ".git/objects/info/alternates").exists(), "oracle clone shares objects")
            module_cache = owned / "modules"
            target_cache = module_cache / "golang.org/x/text@v0.43.0"
            shutil.copytree(args.text_cache, target_cache)
            downloads = args.text_cache.resolve().parents[2] / "cache/download/golang.org/x/text/@v"
            target_downloads = module_cache / "cache/download/golang.org/x/text/@v"
            target_downloads.mkdir(parents=True)
            for suffix in ["mod", "info", "ziphash"]:
                shutil.copy2(downloads / f"v0.43.0.{suffix}", target_downloads)
            environment.update({"GOTOOLCHAIN": "local", "GOPROXY": "off", "GOSUMDB": "off", "GOENV": "off",
                                "GOWORK": "off", "GOCACHE": str(owned / "go-cache"), "GOMODCACHE": str(module_cache),
                                "CGO_ENABLED": "0", "CARGO_TARGET_DIR": str(owned / "rust-target")})
            source = clone / "cmd/qualification-unicode/main.go"
            source.parent.mkdir(parents=True)
            source.write_bytes((HERE / "oracle.go.in").read_bytes())
            go_binary = owned / "unicode-oracle"
            command([args.go, "build", "-trimpath", "-mod=readonly", "-o", go_binary, "./cmd/qualification-unicode"], clone, environment)
            report["goProvenance"] = json.loads(command([go_binary, "-mode", "provenance"], clone, environment))
            command(rust + ["cargo", "build", "--release", "--workspace", "--locked", "--offline", "--target", host], HERE, environment)
            rust_binary = owned / "rust-target" / host / "release/qualification-unicode-compat"
            report["binaries"] = {"goSha256": digest(go_binary.read_bytes()), "rustSha256": digest(rust_binary.read_bytes())}
            fold = clone / "frontend/src/search-casefold.json"
            require(digest(fold.read_bytes()) == FOLD, "wrong full-fold input")
            report["foldSha256"] = FOLD
            report["inputFiles"] = corpus_inputs(owned)
            mode = "source-cases" if args.suite == "baseline" else "mixed-cases"
            rows = json.loads(command([go_binary, "-mode", mode], clone, environment))
            source_count = len(rows)
            if args.suite == "baseline":
                rows += admitted_cases(owned)
            bounded_cases(rows)
            cases = owned / "sequences.json"
            cases.write_text(json.dumps(rows, ensure_ascii=False) + "\n")
            categories = {}
            for row in rows:
                category = row["name"].split("/")[0]
                categories[category] = categories.get(category, 0) + 1
            report["sequenceInputs"] = {"count": len(rows), "sourceDerivedCount": source_count,
                                        "admittedTextCount": len(rows) - source_count,
                                        "sha256": digest(cases.read_bytes()), "categories": categories,
                                        "admittedCases": rows[source_count:],
                                        "producer": "oracle.go.in " + mode,
                                        "cases": rows if args.suite == "mixed" else None}
            if args.suite == "baseline":
                report["scalars"] = paired(go_binary, rust_binary, fold, clone, environment)
            report["sequences"] = paired(go_binary, rust_binary, fold, clone, environment, cases)
            report["languageClosure"] = language_closure(args.go, clone, environment, module_cache)
            report["disposableOriginalFilesVerifiedAfter"] = identities(clone, files)
            report["sourceChangesInDisposableClone"] = command(["git", "status", "--porcelain"], clone).decode()
        report["immutableOracleVerifiedAfter"] = check_oracle(oracle_root, files)
        report["globalDefaultAfter"] = command([rustup, "default"], HERE, environment).decode().strip()
        require(report["globalDefaultBefore"] == report["globalDefaultAfter"], "global default changed")
        report["sourceSha256"] = {str(p.relative_to(HERE)): digest(p.read_bytes()) for p in [
            HERE / "probe.py", HERE / "oracle.go.in", HERE / "Cargo.toml", HERE / "Cargo.lock",
            HERE / "src/lib.rs", HERE / "src/main.rs"]}
        report["status"] = "DIFF" if any(report[k]["status"] == "DIFF" for k in ["scalars", "sequences"] if k in report) else "PASS"
        report["qualification"] = "partial; observed ARM only; no language port, full string-space or toolkit qualification"
    except (RuntimeError, OSError, subprocess.TimeoutExpired) as error:
        report["toolingError"] = str(error)
        raise
    finally:
        args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(report["status"], "scalars", report.get("scalars", {}).get("compared", "not rerun"), "sequences", report["sequences"]["compared"])
    return 2 if report["status"] == "DIFF" else 0


if __name__ == "__main__":
    raise SystemExit(main())
