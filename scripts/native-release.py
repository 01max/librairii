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
from native_release import install, macos, package


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
    linux = commands.add_parser('package-linux', help='assemble a Linux archive from the current verified stage')
    linux.add_argument('--stage', type=Path, required=True)
    linux.add_argument('--output', required=True)
    mac = commands.add_parser('package-macos', help='assemble an explicitly unsigned .app/DMG from the current native stage')
    mac.add_argument('--stage', type=Path, required=True)
    mac.add_argument('--output', required=True)
    installed = commands.add_parser('inspect-package', help='install and inspect final candidate bytes without a GUI')
    installed.add_argument('--directory', type=Path, required=True)
    installed.add_argument('--expected-source', required=True)
    installed.add_argument('--target', choices=TARGETS, required=True)
    installed.add_argument('--output', required=True)
    args = parser.parse_args()
    if args.action == 'plan':
        print(json.dumps(plan(ROOT, args.target, require_clean=False), indent=2))
    elif args.action == 'build':
        destination = build(ROOT, args.target, args.output)
        print(json.dumps({'stage': str(destination), 'stage_complete': True, 'public_release_qualified': False}))
    elif args.action == 'verify-stage':
        print(json.dumps(verify_stage(args.directory), indent=2))
    elif args.action == 'package-linux':
        print(json.dumps(package.linux(ROOT, args.stage, args.output), indent=2))
    elif args.action == 'package-macos':
        print(json.dumps(macos.assemble(ROOT, args.stage, args.output), indent=2))
    elif args.action == 'inspect-package':
        print(json.dumps(install.inspect(ROOT, args.directory, args.expected_source, args.target, args.output), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print('native release refused: ' + str(error), file=sys.stderr)
        sys.exit(1)
