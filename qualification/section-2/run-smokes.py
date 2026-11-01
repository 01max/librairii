#!/usr/bin/env python3
"""Bound every public-API GUI smoke; a hung loop is a failed check."""
import argparse
import pathlib
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=pathlib.Path)
args = parser.parse_args()
binary = args.binary.resolve(strict=True)
for flags in (['--controller-smoke'], ['--widget-smoke'], ['--edit-smoke'], ['--focus-smoke'],
              ['--quit-smoke'], ['--thumbnail-smoke'], ['--size=560x400', '--french', '--smoke'],
              ['--settings-early', '--smoke']):
    print('Running ' + ' '.join(flags), flush=True)
    subprocess.run([str(binary), *flags], check=True, timeout=30)
