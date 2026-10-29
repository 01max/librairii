#!/usr/bin/env python3
"""Source-bound current desktop1000-story native input/loading and cache gates."""
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


scale = module('backend_scale', ROOT / 'scripts/verify-native-scale.py')
native = module('desktop_scale', ROOT / 'qualification/section-16/native.py')


def main():
    if scale.output(['git', 'status', '--porcelain', '--untracked-files=normal']):
        raise ValueError('native scale qualification requires clean committed source')
    target = os.environ['RUST_TARGET']
    if scale.TARGETS[target] != (platform.system(), platform.machine()):
        raise ValueError('matching native host required')
    destination = ROOT / 'target/quality/native-scale/desktop'
    if destination.exists():
        raise ValueError('fresh native scale destination required')
    destination.mkdir()
    source = scale.output(['git', 'rev-parse', 'HEAD'])
    manifest = {'source_commit': source, 'target': target, 'hardware': scale.hardware(), 'complete': False,
                'native_input_method_version': 2,
                'performance_profile': 'production release; opt-level3; debug assertions and overflow checks disabled',
                'compiler': scale.output(['rustc', '-vV']), 'cargo_lock_sha256': scale.digest(ROOT / 'Cargo.lock')}
    with (destination / 'compiler-artifacts.jsonl').open('w') as out, (destination / 'build.log').open('w') as err:
        subprocess.run(['cargo', 'build', '--locked', '--release', '--target', target,
                        '-p', 'librairii-desktop', '--example', 'scale-native', '--message-format=json'], cwd=ROOT, stdout=out, stderr=err, check=True)
    events = [json.loads(s) for s in (destination / 'compiler-artifacts.jsonl').read_text().splitlines()]
    artifacts = [e for e in events if e.get('reason') == 'compiler-artifact' and e.get('target', {}).get('name') == 'scale-native'
                 and e['target']['kind'] == ['example'] and e.get('executable')]
    if len(artifacts) != 1:
        raise ValueError('exact Cargo native observer executable required')
    artifact = artifacts[0]
    if artifact['profile']['opt_level'] != '3' or artifact['profile']['debug_assertions'] or artifact['profile']['overflow_checks']:
        raise ValueError('exact production release profile required')
    binary = destination / 'scale-native'
    shutil.copy2(artifact['executable'], binary)
    manifest['executable'] = {'sha256': scale.digest(binary), 'bytes': binary.stat().st_size, 'profile': artifact['profile']}
    (destination / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    helper = None
    if platform.system() == 'Darwin':
        helper = destination / 'input-macos'
        subprocess.run(['xcrun', 'swiftc', str(ROOT / 'qualification/section-16/input-macos.swift'), '-o', str(helper)], check=True)
    cases = []
    for name, backend in [('gpu-api', 'winit-skia'), ('software', 'winit-skia-software')]:
        cases.append(native.inspect(binary, destination / name, backend, helper))
    if scale.output(['git', 'rev-parse', 'HEAD']) != source or scale.output(['git', 'status', '--porcelain', '--untracked-files=normal']):
        raise ValueError('source changed during native measurement')
    if scale.digest(binary) != manifest['executable']['sha256'] or scale.digest(ROOT / 'Cargo.lock') != manifest['cargo_lock_sha256']:
        raise ValueError('executable/lock changed during native measurement')
    manifest['complete'] = all(c['complete'] for c in cases)
    manifest['files'] = {str(p.relative_to(destination)): scale.digest(p) for p in destination.rglob('*') if p.is_file() and p.name != 'manifest.json'}
    (destination / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    if not manifest['complete']:
        raise ValueError('both actual rendering paths required')
    print(json.dumps({'source': source, 'native_input_method_version': 2, 'native_input_loading_cache': 'passed'}))


if __name__ == '__main__':
    main()
