#!/usr/bin/env python3
"""Skip Go/Wails CI only when every changed path belongs to the Rust rewrite."""
import os
from pathlib import Path
import re
import subprocess

RUST_PREFIXES = ('crates/', 'xtask/', 'fuzz/', '.cargo/', '.config/', 'docs/rewrite/')
RUST_FILES = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'rustfmt.toml',
              'clippy.toml', 'deny.toml', 'openspec'}


def needs_legacy(base, head):
    # Missing history must run the suites. No API path listing/truncation or
    # filename parsing through a shell; deletions and both sides of renames count.
    if not all(re.fullmatch(r'[0-9a-f]{40}', ref or '') and int(ref, 16)
               for ref in (base, head)):
        return True
    diff = subprocess.run(['git', 'diff', '--name-only', '--no-renames', '-z',
                           base, head, '--'], capture_output=True)
    if diff.returncode:
        return True
    paths = [os.fsdecode(path) for path in diff.stdout.split(b'\0') if path]
    return any(path not in RUST_FILES and not path.startswith(RUST_PREFIXES)
               for path in paths)


def main():
    needed = (os.environ.get('EVENT_NAME') == 'workflow_dispatch'
              or needs_legacy(os.environ.get('BASE_SHA'), os.environ.get('HEAD_SHA')))
    value = str(needed).lower()
    with Path(os.environ['GITHUB_OUTPUT']).open('a') as output:
        output.write(f'legacy={value}\n')
    print(f'Go/Wails verification required: {value}')


if __name__ == '__main__':
    main()
