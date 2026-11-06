#!/usr/bin/env python3
"""Index retained release archives offline. Never executes, signs or approves a package."""
import argparse
import json
from pathlib import Path
import sys
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from native_release.evidence import assemble


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True, help='explicitly selected evidence input JSON')
    parser.add_argument('--output', type=Path, required=True, help='new manifest beside the input JSON')
    args = parser.parse_args(argv)
    result = assemble(args.input, args.output)
    print(json.dumps({'manifest': str(args.output), 'source_commit': result['source_commit'],
                      'targets': len(result['targets']), 'acceptance_verified': False,
                      'public_release_qualified': False}, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile) as error:
        print('native evidence refused: ' + str(error), file=sys.stderr)
        sys.exit(1)
