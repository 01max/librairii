#!/usr/bin/env python3
"""Protected manual CI entry point; never called by pull-request builds."""
import argparse
import json
import os
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from native_release import signing
from native_release.files import write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--expected-source', required=True)
    parser.add_argument('--expected-sha256', required=True)
    parser.add_argument('--target', choices=('aarch64-apple-darwin','x86_64-apple-darwin'), required=True)
    parser.add_argument('--team', required=True)
    parser.add_argument('--certificate', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--preflight', action='store_true')
    args = parser.parse_args()
    if (os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('GITHUB_EVENT_NAME') != 'workflow_dispatch'
            or os.environ.get('GITHUB_REF') not in ('refs/heads/main','refs/heads/rewrite-in-rust-native-desktop')):
        raise ValueError('public signing requires protected manual CI from a trusted integration workflow')
    if not re.fullmatch('[A-Z0-9]{10}',args.team) or not re.fullmatch('[0-9A-F]{40}',args.certificate):
        raise ValueError('independently selected team/certificate identities are required')
    signing.selection(args.source_root,args.directory,args.expected_source,args.target,args.expected_sha256)
    if args.preflight:
        print(json.dumps({'source':args.expected_source,'unsigned_sha256':args.expected_sha256,'preflight':True}))
        return
    temporary = Path(os.environ['RUNNER_TEMP']).resolve(strict=True)
    with signing.credentials(temporary,os.environ) as keychain:
        result = signing.sign(args.source_root,args.directory,args.expected_source,args.target,args.expected_sha256,
                              args.output,keychain,'librairii-notary',args.team,args.certificate)
    write_json(Path(args.source_root)/args.output/'credential-retirement.json',
               {'dedicated_keychain_retired':True,'signing_secrets_removed_from_child_environment':True})
    print(json.dumps({'artifact':result['artifact'],'public_release_qualified':False}))


if __name__ == '__main__':
    main()
