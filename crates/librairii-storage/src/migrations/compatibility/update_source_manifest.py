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
LANGUAGE_NOTICES = ["LICENSE-GO-BSD.txt", "PATENTS-GO.txt", "LICENSE-CLDR32-UNICODE.txt", "LICENSE-CLDR32-APACHE.txt", "LICENSE-CC0.txt"]
TABLES = [MODULE + "unicode/norm/tables15.0.0.go"]
PROPERTIES = ["Go1.26.5/src/unicode/tables.go",
              "oracle73a93303/frontend/src/search-casefold.json"]
ALGORITHM = [MODULE + "unicode/norm/" + name for name in [
    "composition.go", "forminfo.go", "normalize.go", "trie.go"]]
ALGORITHM += [MODULE + "cases/fold.go", "Go1.26.5/src/unicode/graphic.go",
              "Go1.26.5/src/unicode/letter.go", "Go1.26.5/src/strings/strings.go",
              "internal/searchtext/searchtext.go"]
LANGUAGE_TABLES = [MODULE + "internal/language/tables.go", MODULE + "internal/language/lookup.go",
                   MODULE + "internal/language/compact/tables.go"]
LANGUAGE_ALGORITHM = [MODULE + "internal/language/" + name for name in ["parse.go", "lookup.go", "language.go", "compact.go", "common.go"]]
LANGUAGE_ALGORITHM += [MODULE + "internal/tag/tag.go", MODULE + "language/parse.go", MODULE + "language/language.go",
                       MODULE + "internal/language/compact/compact.go", MODULE + "internal/language/compact/language.go",
                       "oracle73a93303/internal/library/story_query.go"]


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
        "extract_language.py": ("translated", "Hand-translate frozen getLangID finite two/three-letter lookup and compact bit decoding into literal extraction; verify selected bytes/headers; remove ONLY regionTypes ccTLD bit1/value2 before writing, retain other bits; never copy TLD APIs or regenerate current registries.", LANGUAGE_TABLES + [MODULE + "internal/language/compact.go"]),
        "language_data.rs": ("generated", "python3 extract_language.py --source-root <authenticated49-materialization> --source-index <pinned-index>; cargo fmt -p librairii-storage. Literal frozen mappings, aliases, scripts, regions, variants, grandfathered data and compact core IDs; regionTypes bit1/value2 excluded before emission, other bits unchanged; no TLD APIs or current registry.", LANGUAGE_TABLES),
        "language-cases.json": ("generated", "python3 extract_language.py: decode the773 frozen compact cores and2 special strings, retain112 variant names; append authored suffix/selected/random seed protocol. Qualification INPUTS only, never expected outputs, fixture-specific aliases or preservation corpus regeneration; no DNS data.", LANGUAGE_TABLES + [MODULE + "internal/language/compact.go"]),
        "language.rs": ("translated", "Private product35-byte canonicalLanguage boundary; frozen Default=Deprecated|Legacy scanner/core/variant/extension and exact compact round trip. Lazy token validation preserves transform truncation; region override/compact bit widths remain observable; no SuppressScript/Macro/CLDR, parent/matcher approximation or TLD API. TrimSpace delegates the single private frozen Unicode primitive. Does not expose partial error tags that the product rejects.", LANGUAGE_TABLES + LANGUAGE_ALGORITHM),
        "language_oracle.go.in": ("authored", "Qualification-only calls to actual frozen public Default parser/String/Und/error and product boundary; framed digest protocol, finite alphabets, compact region overrides and deterministic grammar sequences; no runtime Go.", []),
        "verify_language.py": ("authored", "Reuse authenticated disposable source/tool materialization; compare original Go results with a separate owned copy masking exactly ccTLDbit2 and deleting BOTH TLD API functions. Verify exactly3 counterfactual members changed,484 unchanged and all original inputs unchanged. No preservation or production writes.", []),
        "language-reference.json": ("generated", "python3 verify_language.py: actual pinned offline Go1.26.5 Default/String/Und/errors and35-byte product outcomes on1369529 inputs; counterfactual deletion of TLD APIs+masked bit2 gives identical raw/product digests. Retain exact original/modified hashes, source/probe/seed identities and framed results; not all possible strings, Collection SQL or native filesystem qualification.", LANGUAGE_TABLES + LANGUAGE_ALGORITHM),
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
        for original in dict.fromkeys(originals):
            selected = sources[original]
            if original.startswith((MODULE + "internal/language/", MODULE + "internal/tag/", MODULE + "language/")):
                selection = "language-default-without-dns"
            elif original.endswith(("internal/searchtext/searchtext.go", "internal/library/story_query.go")):
                selection = "product-boundary"
            else:
                selection = "normalization"
            uses.append({"path": original, "sha256": selected["sha256"],
                         "header_sha256": selected["first12LinesSha256"], "selection": selection})
            if selection == "normalization":
                notices.update(NORMALIZATION_NOTICES)
            elif selection == "language-default-without-dns":
                notices.update(LANGUAGE_NOTICES)
        result.append({"path": name, "bytes": len(data), "sha256": digest(data),
                       "kind": kind, "recipe": recipe, "sources": uses,
                       "notices": sorted(notices)})
    manifest = {"version": 1, "bundle": BUNDLE, "files": result}
    (directory / "source-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"privateFiles": len(result), "manifestSha256": digest((directory / "source-manifest.json").read_bytes())}))


if __name__ == "__main__":
    main()
