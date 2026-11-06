#!/usr/bin/env python3
"""Install only the selected pinned CI helpers, reusing verified cached versions."""
import os
from pathlib import Path
import subprocess
import sys
import tomllib

GROUPS = {
    'quality': ('cargo-nextest', 'cargo-deny', 'cargo-audit'),
    'focused': ('cargo-nextest', 'cargo-llvm-cov', 'cargo-fuzz'),
}


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in GROUPS:
        raise SystemExit('expected quality or focused helper set')
    install_root = Path(os.environ['CARGO_INSTALL_ROOT'])
    pins = tomllib.loads(Path('.config/rust-tools.toml').read_text())['tools']
    for name in GROUPS[sys.argv[1]]:
        version = pins[name]
        args = ['cargo', name.removeprefix('cargo-'), '--version']
        present = (install_root / 'bin' / name).is_file()
        actual = subprocess.run(args, capture_output=True, text=True) if present else None
        if actual and actual.returncode == 0 and version in actual.stdout.split():
            print(f'{name} {version}: cached', flush=True)
            continue
        subprocess.run(['cargo', 'install', '--locked', '--force', '--version', version, name], check=True)
        actual = subprocess.run(args, capture_output=True, text=True, check=True)
        if version not in actual.stdout.split():
            raise SystemExit(f'{name} did not install pinned version {version}')


if __name__ == '__main__':
    main()
