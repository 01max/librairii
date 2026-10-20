#!/usr/bin/env python3
"""Inventory the explicit, private original-to-derived map after formatting.

Run after extraction, reference generation or handwritten edits. Unknown files
fail instead of silently becoming authored entries. Root policy stays owner5.
"""
import hashlib
import json
from pathlib import Path


BUNDLE = "go-1.26.5-xtext-0.43.0-unicode15-cldr32"
INDEX_SHA = "10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99"
MODULE = "golang.org/x/text@v0.43.0/"
NORMALIZATION_NOTICES = ["LICENSE-GO-BSD.txt", "PATENTS-GO.txt", "LICENSE-UNICODE-V3.txt"]
TABLES = [MODULE + "unicode/norm/tables15.0.0.go"]
PROPERTIES = ["Go1.26.5/src/unicode/tables.go",
              "oracle73a93303/frontend/src/search-casefold.json"]
ALGORITHM = [MODULE + "unicode/norm/" + name for name in [
    "composition.go", "forminfo.go", "normalize.go", "trie.go"]]
ALGORITHM += [MODULE + "cases/fold.go", "Go1.26.5/src/unicode/graphic.go",
              "Go1.26.5/src/unicode/letter.go", "Go1.26.5/src/strings/strings.go",
              "internal/searchtext/searchtext.go"]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    directory = Path(__file__).resolve().parent
    repository = directory.parents[4]
    index_bytes = (repository / ".config/upstream-sources" / BUNDLE / "source-index.json").read_bytes()
    if digest(index_bytes) != INDEX_SHA:
        raise ValueError("changed authenticated source index")
    index = json.loads(index_bytes)
    sources = {f["path"]: f for p in index["packages"] for f in p["files"]}
    sources.update({f["path"]: f for f in index["supplementalSourceFiles"]})
    recipe = "python3 extract_normalization.py --source-root <authenticated49-materialization> --source-index <pinned-index>; cargo fmt -p librairii-storage"
    files = {
        "mod.rs": ("authored", "Private module wiring only; no source algorithm.", []),
        "extract_normalization.py": ("authored", "Extract literal fixed source arrays and the frozen full-fold map, verifying their hashes/headers; no upstream execution or regeneration from current data.", []),
        "update_source_manifest.py": ("authored", "Explicit private file map; hash actual post-format bytes and retain pinned original identities, selections and notice names.", []),
        "verify_normalization.py": ("authored", "Offline disposable exact73 archive/authenticated487-member ZIP/pinnedGo qualification with before/after original-file verification; never a product runtime or corpus generator.", []),
        "normalization_oracle.go.in": ("authored", "Qualification glue calling actual frozen Go APIs; fixed scalar/framing/sequence program, executed only by verify_normalization.py in independent inputs.", []),
        "normalization_data.rs": ("generated", recipe + "; retain only NFKC trie/decomposition/CCC data used by NFKD, no recomposition data or modern source.", TABLES),
        "unicode_properties.rs": ("generated", recipe + "; retain Go15 Mn/White_Space ranges plus exact frozen1616 full-fold map, no current Unicode regeneration.", PROPERTIES),
        "normalization.rs": ("translated", "Hand translation of frozen full-fold→Go stream-safe NFKD→Mn-only removal→Go whitespace folding; original-character leading/trailing partition precedes stable CCC insertion. Safe checked UTF8 trie and fixed32-rune segment, Hangul decomposition; no modern composition or runtime Go.", TABLES + PROPERTIES + ALGORITHM),
        "normalization-reference.json": ("generated", "python3 verify_normalization.py; actual pinned Go1.26.5 frozen Normalize/cases.Fold/norm.NFKD/unicode APIs over all1112064 valid scalars and147228 boundary sequences. Includes57 CCC/Mn representatives and all18019 decomposing/backward-interacting scalars on both sides of overflow; exact framed SHA256 digests. Source/tool/ZIP/probe identities and before/after proof retained; not arbitrary-string or language qualification.", TABLES + PROPERTIES + ALGORITHM),
    }
    actual = {p.relative_to(directory).as_posix() for p in directory.rglob("*")
              if p.is_file() and p.name != "source-manifest.json"}
    if actual != set(files):
        raise ValueError("explicit private inventory mismatch: " + repr(actual ^ set(files)))
    result = []
    for name in sorted(files):
        kind, recipe, originals = files[name]
        path = directory / name
        if path.is_symlink():
            raise ValueError("private derivatives cannot be aliases")
        data = path.read_bytes()
        uses = []
        notices = set()
        for original in originals:
            selected = sources[original]
            selection = "product-boundary" if original == "internal/searchtext/searchtext.go" else "normalization"
            uses.append({"path": original, "sha256": selected["sha256"],
                         "header_sha256": selected["first12LinesSha256"], "selection": selection})
            if selection == "normalization":
                notices.update(NORMALIZATION_NOTICES)
        result.append({"path": name, "bytes": len(data), "sha256": digest(data),
                       "kind": kind, "recipe": recipe, "sources": uses,
                       "notices": sorted(notices)})
    manifest = {"version": 1, "bundle": BUNDLE, "files": result}
    (directory / "source-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"privateFiles": len(result), "manifestSha256": digest((directory / "source-manifest.json").read_bytes())}))


if __name__ == "__main__":
    main()
