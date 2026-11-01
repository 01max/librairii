#!/usr/bin/env python3
"""Preview a native package build; execute only with --execute. Never opens the GUI."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from native_release.files import fresh_output
from native_release.identity import TARGETS, plan


def commands(target, name):
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,63}', name):
        raise ValueError('name must contain 1–64 letters, digits, underscores or hyphens')
    directory = ROOT / 'target/native-release' / name
    entry = [sys.executable, '-I', '-B', str(ROOT / 'scripts/native-release.py')]
    packaging = 'package-macos' if target.endswith('apple-darwin') else 'package-linux'
    return directory, [
        entry + ['build', '--target', target, '--output', str(directory / 'stage')],
        entry + ['verify-stage', str(directory / 'stage')],
        entry + [packaging, '--stage', str(directory / 'stage'), '--output', str(directory / 'package')],
    ]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=TARGETS, required=True)
    parser.add_argument('--name', required=True, help='unused build name below target/native-release')
    parser.add_argument('--execute', action='store_true', help='build, verify and assemble the previewed unsigned candidate')
    args = parser.parse_args(argv)
    directory, steps = commands(args.target, args.name)
    selected = plan(ROOT, args.target, require_clean=args.execute)
    print(json.dumps({'inputs': selected, 'output': str(directory), 'commands': steps,
                      'execute': args.execute, 'public_release_qualified': False}, indent=2), flush=True)
    if args.execute:
        fresh_output(ROOT, directory)
        for command in steps:
            subprocess.run(command, cwd=ROOT, check=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print('native package refused: ' + str(error), file=sys.stderr)
        sys.exit(1)
