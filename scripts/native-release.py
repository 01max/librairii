#!/usr/bin/env python3
"""The cargo xtask release facade. Never launches the product GUI."""
import argparse
import json
import subprocess
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from native_release.build import build, verify_stage
from native_release.identity import TARGETS, plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='action', required=True)
    planning = commands.add_parser('plan', help='show pinned inputs; does not qualify or build an artifact')
    planning.add_argument('--target', choices=TARGETS, required=True)
    staging = commands.add_parser('build', help='build and seal a production payload; does not sign or qualify it')
    staging.add_argument('--target', choices=TARGETS, required=True)
    staging.add_argument('--output', required=True, help='new directory below target/native-release')
    checking = commands.add_parser('verify-stage', help='verify all staged final bytes without launching the GUI')
    checking.add_argument('directory', type=Path)
    args = parser.parse_args()
    if args.action == 'plan':
        print(json.dumps(plan(ROOT, args.target, require_clean=False), indent=2))
    elif args.action == 'build':
        destination = build(ROOT, args.target, args.output)
        print(json.dumps({'stage': str(destination), 'stage_complete': True, 'public_release_qualified': False}))
    else:
        print(json.dumps(verify_stage(args.directory), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print('native release refused: ' + str(error), file=sys.stderr)
        sys.exit(1)
