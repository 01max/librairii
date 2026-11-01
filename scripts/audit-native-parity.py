#!/usr/bin/env python3
"""Observe named parity checks in retained archives, without native execution."""
import argparse
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from native_release.parity import audit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True, help='Existing three-target evidence input plan')
    parser.add_argument('--output', type=Path, required=True, help='New JSON beside the input plan')
    args = parser.parse_args()
    result = audit(args.input, args.output)
    print(f"Observed {len(result['rows'])} parity rows across {len(result['targets'])} targets; release acceptance remains pending.")


if __name__ == '__main__':
    main()
