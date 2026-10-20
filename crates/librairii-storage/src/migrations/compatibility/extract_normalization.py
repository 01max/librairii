#!/usr/bin/env python3
"""Extract selected immutable Go15 tables, never execute upstream code.

Inputs are the ROOT-approved authenticated source materialization. This is a
private data translation recipe, not a preservation-corpus/fixture generator.
Every selected byte/header is checked against the fixed source index first.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

INDEX_HASH = "10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99"
MODULE = "golang.org/x/text@v0.43.0/"


def body(source, start):
    offset = source.index("{", source.index(start))
    depth = 1
    end = offset + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return re.sub(r"//[^\n]*", "", source[offset + 1:end - 1])


def numbers(source, name, length=None):
    result = [0] * length if length is not None else []
    position = 0
    literal = body(source, "var " + name + " =")
    for item in literal.split(","):
        item = item.strip()
        if not item:
            continue
        if ":" in item:
            key, item = item.split(":")
            position = int(key, 0)
        value = int(item, 0)
        if length is None:
            if position != len(result):
                raise ValueError("unexpected sparse literal position")
            result.append(value)
        else:
            result[position] = value
        position += 1
    return result


def rust_string(value):
    return '"' + "".join(c if 32 <= ord(c) < 127 and c not in '\\"'
                         else "\\u{" + format(ord(c), "x") + "}" for c in value) + '"'


def array(name, values, kind):
    rows = [f"pub(super) const {name}: &[{kind}] = &["]
    rows += ["    " + ", ".join(hex(v) for v in values[i:i+12]) + ","
             for i in range(0, len(values), 12)]
    return "#[rustfmt::skip]\n" + "\n".join(rows + ["];", ""])


def main():
    cli = argparse.ArgumentParser()
    cli.add_argument("--source-root", required=True, type=Path)
    cli.add_argument("--source-index", required=True, type=Path)
    args = cli.parse_args()
    index_bytes = args.source_index.read_bytes()
    if hashlib.sha256(index_bytes).hexdigest() != INDEX_HASH:
        raise ValueError("changed authenticated source index")
    index = json.loads(index_bytes)
    records = {f["path"]: f for p in index["packages"] for f in p["files"]}
    records.update({f["path"]: f for f in index["supplementalSourceFiles"]})
    used = {}

    def read(name):
        data = (args.source_root / name).read_bytes()
        record = records[name]
        if (len(data), hashlib.sha256(data).hexdigest()) != (record["bytes"], record["sha256"]):
            raise ValueError("source differs from authenticated index")
        header = b"".join(data.splitlines(keepends=True)[:12])
        if header.hex() != record["headerFirst12LinesHex"]:
            raise ValueError("source header differs from authenticated index")
        used[name] = record
        return data.decode("utf-8")

    tables = read(MODULE + "unicode/norm/tables15.0.0.go")
    source_header = "/* Original upstream header (retained verbatim):\n" + "\n".join(tables.splitlines()[:12]) + "\n*/"
    result = [source_header,
              "// PRIVATE DERIVATIVE, not an upstream file. Unicode15 NFKD table subset.",
              "// Recipe: extract_normalization.py; source-manifest.json retains identities/notices."]
    for name in ["firstCCC", "firstLeadingCCC", "firstStarterWithNLead", "firstCCCZeroExcept"]:
        value = re.search(r"\b" + name + r"\s*=\s*(0x[0-9A-Fa-f]+)", tables)[1]
        if name == "firstCCCZeroExcept":
            result.append("#[cfg(test)]")
        result.append(f"pub(super) const {name.upper()}: usize = {value};")
    for go_name, rust_name, count, kind in [
        ("ccc", "CCC", 56, "u8"),
        ("decomps", "DECOMPS", 19426, "u8"),
        ("nfkcValues", "VALUES", 6208, "u16"),
        ("nfkcIndex", "INDEX", 1408, "u16"),
        ("nfkcSparseOffset", "SPARSE_OFFSET", 176, "u16"),
    ]:
        values = numbers(tables, go_name, None if go_name in ["decomps", "nfkcSparseOffset"] else count)
        if len(values) != count:
            raise ValueError("changed selected normalization table size")
        result.append(array(rust_name, values, kind))
    sparse = []
    for record in re.findall(r"\{([^{}]+)\}", body(tables, "var nfkcSparseValues =")):
        fields = dict(re.findall(r"(value|lo|hi):\s*(0x[0-9a-fA-F]+|\d+)", record))
        sparse.append(tuple(int(fields.get(key, "0"), 0) for key in ["value", "lo", "hi"]))
    if len(sparse) != 919:
        raise ValueError("changed selected normalization sparse range count")
    result += ["#[rustfmt::skip]", "pub(super) const SPARSE: &[(u16, u8, u8)] = &["]
    result += ["    (" + ", ".join(hex(v) for v in row) + ")," for row in sparse]
    result.append("];\n")
    destination = Path(__file__).resolve().parent
    (destination / "normalization_data.rs").write_text("\n".join(result), encoding="utf-8")

    go_unicode = read("Go1.26.5/src/unicode/tables.go")
    properties = ["// Code generated from the approved Go1.26.5 Unicode15 source; PRIVATE DERIVATIVE.",
                  "/* Original upstream header (retained verbatim):"]
    properties += go_unicode.splitlines()[:12] + ["*/"]
    for name, rust_name in [("_Mn", "MN"), ("_White_Space", "WHITE_SPACE")]:
        table = body(go_unicode, "var " + name + " =")
        ranges = []
        for fields in re.findall(r"\{\s*(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+),\s*(\d+)\s*\}", table):
            ranges.append(tuple(int(v, 0) for v in fields))
        if ranges != sorted(ranges) or not ranges:
            raise ValueError("changed Go Unicode range ordering")
        properties += ["#[rustfmt::skip]", f"pub(super) const {rust_name}: &[(u32, u32, u32)] = &["]
        properties += ["    (" + ", ".join(hex(v) for v in row) + ")," for row in ranges]
        properties.append("];\n")
    fold = json.loads(read("oracle73a93303/frontend/src/search-casefold.json"))
    if len(fold) != 1616 or not all(len(k) == 1 for k in fold):
        raise ValueError("changed frozen full-fold map")
    properties += ["#[rustfmt::skip]", "pub(super) const FULL_FOLD: &[(u32, &str)] = &["]
    properties += [f"    ({hex(ord(k))}, {rust_string(v)})," for k, v in sorted(fold.items())]
    properties.append("];\n")
    (destination / "unicode_properties.rs").write_text("\n".join(properties), encoding="utf-8")

    # Include all actual algorithm sources referenced by the handwritten port.
    for name in ["unicode/norm/composition.go", "unicode/norm/forminfo.go",
                 "unicode/norm/normalize.go", "unicode/norm/trie.go", "cases/fold.go"]:
        read(MODULE + name)
    read("internal/searchtext/searchtext.go")
    read("Go1.26.5/src/unicode/graphic.go")
    read("Go1.26.5/src/unicode/letter.go")
    read("Go1.26.5/src/strings/strings.go")
    print(json.dumps({"extracted": ["normalization_data.rs", "unicode_properties.rs"],
                      "verifiedOriginals": len(used), "sourceIndexSha256": INDEX_HASH}))


if __name__ == "__main__":
    main()
