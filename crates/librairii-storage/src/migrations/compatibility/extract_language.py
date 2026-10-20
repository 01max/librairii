#!/usr/bin/env python3
"""Translate only frozen Default-parser lookup data, excluding DNS flags/APIs.

All inputs must match the authenticated49-file index. This does not run the
upstream generator or use a present-day language registry. ccTLD bit1/value2
is removed BEFORE any region flags are written to the private derivative.

Original notices for the translated lookup/compact algorithms:
// Copyright 2013 The Go Authors. All rights reserved.
// Copyright 2018 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
"""
import argparse
import ast
import hashlib
import itertools
import json
from pathlib import Path
import re
import runpy


INDEX_HASH = "10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99"
MODULE = "golang.org/x/text@v0.43.0/"


def string_value(source, name):
    start = re.search(r"\b" + name + r"\b[^=\n]*=", source).end()
    result = bytearray()
    while True:
        skip = re.match(r"\s*(?://[^\n]*\n\s*)*", source[start:])
        start += skip.end()
        match = re.match(r'"(?:\\.|[^"\\])*"', source[start:])
        if match is None:
            raise ValueError("expected selected Go string literal")
        result.extend(ast.literal_eval(match[0]).encode("latin1"))
        start += match.end()
        rest = re.match(r"\s*(?://[^\n]*\n\s*)*", source[start:])
        start += rest.end()
        if source[start:start+1] != "+":
            return bytes(result)
        start += 1


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

    def read(name):
        data = (args.source_root / name).read_bytes()
        record = records[name]
        if (len(data), hashlib.sha256(data).hexdigest()) != (record["bytes"], record["sha256"]):
            raise ValueError("changed selected source bytes")
        if b"".join(data.splitlines(keepends=True)[:12]).hex() != record["headerFirst12LinesHex"]:
            raise ValueError("changed selected source header")
        return data.decode("utf-8")

    own = Path(__file__).resolve().parent
    helpers = runpy.run_path(str(own / "extract_normalization.py"))
    body, numbers, array = (helpers[name] for name in ["body", "numbers", "array"])
    tables = read(MODULE + "internal/language/tables.go")
    lookup = read(MODULE + "internal/language/lookup.go")
    compact = read(MODULE + "internal/language/compact/tables.go")
    lang = string_value(tables, "lang")
    alt_lang = string_value(tables, "altLangISO3")
    alt_ids = numbers(tables, "altLangIndex", 6)
    no_index = numbers(tables, "langNoIndex", 2197)
    if len(lang) != 5324:
        raise ValueError("changed fixed language index")
    offset = int(re.search(r"const langNoIndexOffset = (\d+)", tables)[1])
    noncanonical_und = int(re.search(r"const nonCanonicalUnd = (\d+)", tables)[1])
    canonical, iso2, noncanonical = {}, {}, {}
    for i in range(0, len(lang) - 4, 4):
        entry = lang[i:i+4]
        if entry[3] == 0:
            canonical[entry[:3]] = 0 if i // 4 == noncanonical_und else i // 4
        else:
            iso2[entry[:2]] = i // 4
            noncanonical[entry[:1] + entry[2:4]] = i // 4
    alternatives = {alt_lang[i:i+3]: alt_ids[alt_lang[i+3]]
                    for i in range(4, len(alt_lang) - 4, 4)}
    language_ids = []
    for size in [2, 3]:
        for chars in itertools.product(range(ord("a"), ord("z") + 1), repeat=size):
            key = bytes(chars)
            value = None
            if size == 2:
                value = iso2.get(key)
            else:
                number = ((chars[0] - 97) * 26 + chars[1] - 97) * 26 + chars[2] - 97
                if key in canonical:
                    value = canonical[key]
                elif key in alternatives:
                    value = alternatives[key]
                elif no_index[number // 8] & (1 << (number % 8)):
                    value = number + offset
                else:
                    value = noncanonical.get(key)
            if value is not None:
                language_ids.append((key.decode("ascii"), value))
    language_ids.sort()

    def lang_string(value):
        if value == 0:
            return "und"
        if value >= offset:
            value -= offset
            return "".join(chr(97 + value // n % 26) for n in [676, 26, 1])
        entry = lang[value * 4:value * 4 + 4]
        return entry[:3 if entry[3] == 0 else 2].decode("ascii")

    def pairs(source, name):
        return [(int(a, 0), int(b, 0)) for a, b in re.findall(
            r"\{From:\s*(0x[0-9a-f]+|\d+), To:\s*(0x[0-9a-f]+|\d+)\}",
            body(source, "var " + name + " ="))]

    aliases = pairs(tables, "AliasMap")
    alias_types = numbers(tables, "AliasTypes", 193)
    if len(aliases) != 193:
        raise ValueError("changed frozen language alias count")
    # No unmasked regionTypes data is written, including comments/manifests.
    region_flags = [v & ~2 for v in numbers(tables, "regionTypes", 359)]
    if any(v & ~5 for v in region_flags):
        raise ValueError("unknown frozen non-DNS region flag")
    declarations = [
        "// Code generated by extract_language.py from the approved frozen CLDR32 source; PRIVATE DERIVATIVE.",
        "// Copyright 2013 The Go Authors. All rights reserved.",
        "// Use of this source code is governed by a BSD-style",
        "// license that can be found in the LICENSE file.",
        "// ccTLD bit1/value2 and BOTH TLD APIs are EXCLUDED; other region bits retained.",
        "/* Original tables header (retained verbatim):",
        *tables.splitlines()[:12], "*/",
        f"pub(super) const LANG_NO_INDEX_OFFSET: u16 = {offset};",
        array("LANG", list(lang), "u8"),
        "#[rustfmt::skip]", "pub(super) const LANGUAGE_IDS: &[(&str, u16)] = &["]
    declarations += [f'    ("{key}", {value}),' for key, value in language_ids]
    declarations += ["];", "#[rustfmt::skip]", "pub(super) const LANGUAGE_ALIASES: &[(u16, u16, u8)] = &["]
    declarations += [f"    ({a}, {b}, {t})," for (a, b), t in zip(aliases, alias_types)]
    declarations += ["];", array("SCRIPTS", list(string_value(tables, "script")), "u8"),
                     array("REGION_ISO", list(string_value(tables, "regionISO")), "u8"),
                     array("ALT_REGION_ISO3", list(string_value(tables, "altRegionISO3")), "u8"),
                     array("ALT_REGION_IDS", numbers(tables, "altRegionIDs", 11), "u16"),
                     array("REGION_M49", numbers(tables, "m49", 359), "u16"),
                     "#[cfg(test)]\n" + array("REGION_TYPES_WITHOUT_TLD", region_flags, "u8")]
    m49_index = numbers(tables, "m49Index", 9)
    from_m49 = numbers(tables, "fromM49", 333)
    numeric = []
    for group in range(8):
        for entry in from_m49[m49_index[group]:m49_index[group+1]]:
            code, region = (group << 7) + (entry >> 9), entry & 511
            if code != 0:
                numeric.append((code, region))
    if numeric != sorted(set(numeric)):
        raise ValueError("changed canonical numeric region mapping")
    declarations += ["#[rustfmt::skip]", "pub(super) const NUMERIC_REGIONS: &[(u16, u16)] = &["]
    declarations += [f"    ({code}, {region})," for code, region in numeric]
    declarations += ["];", "#[rustfmt::skip]", "pub(super) const REGION_ALIASES: &[(u16, u16)] = &["]
    declarations += [f"    ({a}, {b})," for a, b in pairs(tables, "regionOldMap")]
    declarations += ["];", "#[rustfmt::skip]", "pub(super) const VARIANTS: &[(&str, u8)] = &["]
    variants = re.findall(r'"([a-z0-9]+)":\s*(0x[0-9a-f]+|\d+)', body(tables, "var variantIndex ="))
    declarations += [f'    ("{key}", {int(value, 0)}),' for key, value in variants]
    declarations += ["];", "#[rustfmt::skip]", "pub(super) const GRANDFATHERED: &[(&str, &str)] = &["]
    constant_ids = {k: int(v, 0) for k, v in re.findall(r"\b(_[A-Za-z0-9]+)\s*=\s*(0x[0-9a-fA-F]+|\d+)", tables)}
    alt_offsets = [int(v) for v in re.findall(r"\d+", body(lookup, "altTagIndex ="))]
    alt_tags = string_value(lookup, "altTags").decode("ascii")
    grandfathered = []
    for key, value in re.findall(r"\[maxLen\]byte\{([^}]+)\}:\s*(_[a-z]+|-?\d+),", lookup):
        key = "".join(ast.literal_eval(v.strip()) for v in key.split(","))
        value = constant_ids[value] if value.startswith("_") else int(value)
        replacement = (alt_tags[alt_offsets[-value-1]:alt_offsets[-value]]
                       if value < 0 else lang_string(value))
        grandfathered.append((key, replacement))
    if len(grandfathered) != 28:
        raise ValueError("changed grandfathered selection")
    declarations += [f'    ("{key}", "{value}"),' for key, value in sorted(grandfathered)]
    declarations += ["];", array("COMPACT_CORES", numbers(compact, "coreTags"), "u32")]
    declarations += ["// Exact compact special-tag strings; no parent/matcher/TLD data.",
                     'pub(super) const COMPACT_SPECIAL: &str = "' + string_value(compact, "specialTagsStr").decode("ascii") + '";', ""]
    output = own / "language_data.rs"
    output.write_text("\n".join(declarations), encoding="utf-8")
    region_iso = string_value(tables, "regionISO")
    scripts = string_value(tables, "script")
    m49 = numbers(tables, "m49", 359)

    def core_string(encoded):
        language, script, region = encoded >> 20, (encoded >> 12) & 255, encoded & 1023
        result = lang_string(language)
        if script:
            result += "-" + scripts[script*4:script*4+4].decode("ascii")
        if region:
            result += "-" + (f"{m49[region]:03d}" if region < 32 else region_iso[(region-32)*4:(region-32)*4+2].decode("ascii"))
        return result

    cores = [core_string(v) for v in numbers(compact, "coreTags")]
    cores += string_value(compact, "specialTagsStr").decode("ascii").split(" ")
    cases = {
        "version": 1,
        "scope": "private Default35-byte product-boundary qualification seeds; no expected outputs or DNS data",
        "compactCores": cores,
        "variants": [key for key, _ in variants],
        "suffixes": ["", "-US", "-MD", "-RO", "-BU", "-SU", "-ZZ", "-Qaai", "-Latn", "-Cyrl",
                     "-Latn-USA", "-u-rg-uszzzz", "-u-rg-gbzzzz", "-Zmth", "-Zsye", "-Zsym", "-Zxxx", "-Zyyy", "-Zzzz"],
        "selected": ["", "und", "root", "und-Zzzz", "und-ZZ", "und-x-user", "x-user", "x", "i-default", "i-enochian",
                     "sh", "sh-Cyrl", "mo", "mo-RO", "iw", "en_US_POSIX", "en-us-posix", "de-posix", "art-lojban",
                     "de-1901-1996", "de-1996-1901", "de-1901-1901", "sl-rozaj-biske-1994", "en-abc", "en-und",
                     "en-u-foobar-foobaz", "en-u-foobaz-foobar", "en-u-abc-abc", "en-u-ca-ca", "en-u-ca-true-ca-true",
                     "en-u-ca-true-ca-false", "en-u-ca-gregory-ca-buddhist", "en-u-nu-latn-ca-gregory", "en-u-12-123",
                     "en-a-foo-a-bar", "en-x-a-u-foo", "en-t-eng", "en-t-sh", "en-t-en-abc", "en-t-en-US-123-!!!",
                     "en-t-en-US-123-foo-%%%%", "en-t-en-US-123", "en-t-en-US-foo", "en-t-12-foo-34", "en-t-h0", "en-t-en-us-h0-foo",
                     "en-US-u-rg-uszzzz", "en-US-u-rg-uszzzz-foobar", "en-u-rg-uszzzz-foobar", "en-US-u-rg-gbzzzz-va-posix",
                     "en-US-u-rg-uszzzz-va-posix", "ca-ES-valencia-u-rg-eszzzz", "ks-Zzzz", "en-Zzzz", "en-Zyyy", "en-Zinh",
                     "\u0085 en_US \u2003", "en-\u212a", "en\u0000", " en ", "en--US", "en_", "_en", "en-1234", "qaa-Qaaa-QM-x-a",
                     "en-u-" + "a"*30, "en-u-" + "a"*31, "en-" + "a"*8, "en-" + "a"*9],
        "randomTokens": ["en", "und", "US", "001", "abc", "Qaai", "Zzzz", "fonipa", "biske", "1901", "", "x", "u", "t", "a", "9",
                         "ca", "rg", "va", "nu", "true", "foo", "bar", "gregory", "uszzzz", "h0", "12", "ab3", "\u00e9", "#", "_"],
        "sourceCopyright": ["// Copyright 2013 The Go Authors. All rights reserved.",
                            "// Copyright 2018 The Go Authors. All rights reserved."],
    }
    (own / "language-cases.json").write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"privateLookupKeys": len(language_ids), "variants": len(variants),
                      "grandfathered": len(grandfathered), "maskedRegionFlags": len(region_flags),
                      "bytes": output.stat().st_size, "sha256": hashlib.sha256(output.read_bytes()).hexdigest()}))


if __name__ == "__main__":
    main()
