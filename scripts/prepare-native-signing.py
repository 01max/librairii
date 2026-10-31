#!/usr/bin/env python3
"""Print a manual signing dispatch command. Performs no network, signing or secret operations."""
import argparse
import json
import re
import shlex


def hexadecimal(length):
    def parse(value):
        if not re.fullmatch(r'[0-9a-fA-F]{' + str(length) + '}', value):
            raise argparse.ArgumentTypeError(f'exactly {length} hexadecimal characters required')
        return value.lower()
    return parse


def run_id(value):
    if not re.fullmatch(r'[1-9][0-9]*', value):
        raise argparse.ArgumentTypeError('positive GitHub workflow run ID required')
    return value


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=hexadecimal(40), required=True)
    parser.add_argument('--candidate-run', type=run_id, required=True)
    parser.add_argument('--target', choices=['aarch64-apple-darwin', 'x86_64-apple-darwin'], required=True)
    parser.add_argument('--unsigned-sha256', type=hexadecimal(64), required=True, help='reviewed inner DMG hash, not the provider ZIP hash')
    parser.add_argument('--trusted-ref', choices=['main', 'rewrite-in-rust-native-desktop'], default='rewrite-in-rust-native-desktop')
    args = parser.parse_args(argv)
    command = ['gh', 'workflow', 'run', 'rust-sign-macos.yml', '--repo', '01max/librairii',
               '--ref', args.trusted_ref]
    for key, value in [('source', args.source), ('candidate_run', args.candidate_run),
                       ('target', args.target), ('unsigned_sha256', args.unsigned_sha256)]:
        command += ['--raw-field', key + '=' + value]
    print(json.dumps({'command': shlex.join(command), 'argv': command,
                      'dispatched': False, 'inputs_verified': False,
                      'public_release_qualified': False,
                      'next': 'Independently review the candidate and trusted workflow, configure the protected environment, then run this command yourself and approve its signing job.'}, indent=2))


if __name__ == '__main__':
    main()
