#!/usr/bin/env python3
"""Record native host/package facts and operator observations; never infer a pass."""
import argparse
import datetime
import hashlib
import json
import os
import pathlib
import platform
import subprocess

CHECKS = ('keyboard_focus', 'screen_reader', 'french_dead_keys', 'cjk_ime',
          'clipboard', 'scaling_contrast_motion', 'pickers_parenting_cancel',
          'menus_early_settings', 'reveal', 'gpu_rendering', 'gpu_failure_cpu',
          'performance', 'clean_host_launch')

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            h.update(block)
    return h.hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    parser.add_argument('--operator', required=True, help='Name or review handle; do not include private machine paths')
    args = parser.parse_args()
    root = args.package.resolve(strict=True)
    manifest = json.loads((root / 'manifest.json').read_text())
    for name, expected in manifest['files'].items():
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts:
            parser.error('unsafe manifest member')
        path = root.joinpath(*relative.parts)
        if not path.resolve().is_relative_to(root) or any(parent.is_symlink() for parent in (path, *path.parents) if parent != root) or not path.is_file() or digest(path) != expected:
            parser.error('package bytes changed: ' + name)
    if manifest['executable'] not in manifest['files']:
        parser.error('executable is not a verified package member')
    binary = root / manifest['executable']
    if digest(binary) != manifest['binary_sha256']:
        parser.error('binary digest mismatch')
    target = manifest['target']
    required = {'aarch64-apple-darwin': ('Darwin','arm64'),
                'x86_64-apple-darwin': ('Darwin','x86_64'),
                'x86_64-unknown-linux-gnu': ('Linux','x86_64')}
    if (platform.system(), platform.machine()) != required[target]:
        parser.error('wrong native host for this package')
    facts = {'system': platform.system(), 'release': platform.release(),
             'machine': platform.machine(), 'session': os.environ.get('XDG_SESSION_TYPE', 'unknown'),
             'desktop': os.environ.get('XDG_CURRENT_DESKTOP', 'unknown')}
    if platform.system() == 'Darwin':
        bundle = next((p for p in binary.parents if p.suffix == '.app' and p.is_relative_to(root)), None)
        if bundle is not None:
            signature = subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict',
                str(bundle)], capture_output=True, text=True, timeout=30)
            if signature.returncode != 0:
                parser.error('macOS bundle signature is invalid; obtain a rebuilt package')
            facts['bundle_integrity'] = 'verified'
            facts['gatekeeper_acceptance'] = 'not_assessed'
        translated = subprocess.run(['/usr/sbin/sysctl','-in','sysctl.proc_translated'], capture_output=True, text=True)
        facts['rosetta'] = translated.stdout.strip() == '1'
        if facts['rosetta']:
            parser.error('Rosetta is supplemental evidence, not a native Intel qualification')
        facts['os'] = subprocess.check_output(['/usr/bin/sw_vers','-productVersion'], text=True).strip()
    record = {'schema': 1, 'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
              'target': target, 'binary_sha256': manifest['binary_sha256'], 'operator': args.operator,
              'time_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'host': facts,
              'evidence_kind': 'operator_record_template', 'toolkit_selected': False,
              'checks': {name: {'status': 'not_run', 'observations': '', 'capture_references': []} for name in CHECKS}}
    with args.output.open('x') as stream:
        json.dump(record, stream, indent=2)
        stream.write('\n')
    print('Created a pending observation record. Fill actual observations; see operator-checks.md.')

if __name__ == '__main__':
    main()
