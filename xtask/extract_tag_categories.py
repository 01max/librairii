#!/usr/bin/env python3
"""Extract ONLY Go1.26.5 Unicode15 Letter/Nd ranges from authenticated source.

Usage: python3 xtask/extract_tag_categories.py /path/to/Go1.26.5/src/unicode/tables.go
Writes the private table leaf; never changes a reference fixture or upstream file.
"""
from pathlib import Path
import hashlib
import re
import sys

SOURCE_SHA = '6a88d48746c4b2c471cc04ae40559694b734770212e6f5ea5f1962adb0c0bdb1'


def extract(source):
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA:
        raise ValueError('changed frozen Go Unicode15 source')
    text = source.decode('utf-8')
    output = ('// PRIVATE DERIVATIVE: Go1.26.5 Unicode15 Letter and Nd ranges.\n'
              '// Copyright 2009 The Go Authors. All rights reserved.\n'
              '// BSD-style license: .config/upstream-notices/go-1.26.5-xtext-0.43.0-unicode15-cldr32/LICENSE-GO-BSD.txt\n'
              '// Source SHA-256: ' + SOURCE_SHA + '\n'
              '// Recipe: xtask/extract_tag_categories.py; original source is never modified.\n')
    for name, label in [('_L', 'LETTER'), ('_Nd', 'DIGIT')]:
        start = text.index('{', text.index('var ' + name + ' ='))
        depth, end = 1, start + 1
        while depth:
            depth += (text[end] == '{') - (text[end] == '}')
            end += 1
        ranges = [tuple(int(v, 0) for v in row) for row in re.findall(
            r'\{\s*(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+),\s*(\d+)\s*\}', text[start:end])]
        if not ranges or any(a[1] >= b[0] for a, b in zip(ranges, ranges[1:])):
            raise ValueError('changed frozen range representation')
        output += '#[rustfmt::skip]\npub(super) const ' + label + ': &[(u32,u32,u32)] = &[\n'
        output += ''.join(f'    ({lo:#x}, {hi:#x}, {stride:#x}),\n' for lo, hi, stride in ranges)
        output += '];\n'
    return output


if __name__ == '__main__':
    root = Path(__file__).resolve().parent.parent
    (root / 'crates/librairii-core/src/tagging/unicode15.rs').write_text(
        extract(Path(sys.argv[1]).read_bytes()), encoding='utf-8')
