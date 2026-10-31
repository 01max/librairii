#!/usr/bin/env python3
"""Verify a pinned independent oracle without executing its project code."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('oracle', type=Path)
args = parser.parse_args()
manifest = json.loads((Path(__file__).parent.parent / 'oracle-source.json').read_text())
root = args.oracle.resolve(strict=True)
head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
if head != manifest['commit']:
    raise SystemExit(f'wrong oracle: {head}')
if subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain', '--untracked-files=no']):
    raise SystemExit('oracle tracked source is dirty')
for entry in manifest['files']:
    target = root / entry['path']
    if target.is_symlink() or hashlib.sha256(target.read_bytes()).hexdigest() != entry['sha256']:
        raise SystemExit(f'oracle source mismatch: {entry["path"]}')
print(f'verified {len(manifest["files"])} files at {head}')
