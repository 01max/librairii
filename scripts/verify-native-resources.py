#!/usr/bin/env python3
"""Current release resource/package and native display qualification; no publication."""
import importlib.util
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


scale = module('backend_scale', ROOT / 'scripts/verify-native-scale.py')
native = module('desktop_scale', ROOT / 'qualification/section-16/native.py')
resources = module('current_resources', ROOT / 'qualification/section-16/resources.py')
display = module('current_display', ROOT / 'qualification/section-16/display.py')


def main():
    if scale.output(['git', 'status', '--porcelain', '--untracked-files=normal']):
        raise ValueError('resource qualification requires clean committed source')
    target = os.environ['RUST_TARGET']
    if scale.TARGETS[target] != (platform.system(), platform.machine()):
        raise ValueError('matching native resource host required')
    subprocess.run([sys.executable, '-I', '-B', str(ROOT / 'qualification/section-16/test_resources.py')], check=True)
    subprocess.run([sys.executable, '-I', '-B', str(ROOT / 'qualification/section-16/test_resource_linux.py')], check=True)
    destination = ROOT / 'target/quality/native-scale/resources'
    if destination.exists():
        raise ValueError('fresh resource evidence destination required')
    destination.mkdir()
    source = scale.output(['git', 'rev-parse', 'HEAD'])
    report = {'source_commit': source, 'target': target, 'complete': False, 'hardware': scale.hardware(),
              'cargo_lock_sha256': scale.digest(ROOT / 'Cargo.lock'), 'compiler': scale.output(['rustc', '-vV'])}
    with (destination / 'compiler-artifacts.jsonl').open('w') as out, (destination / 'build.log').open('w') as err:
        subprocess.run(['cargo', 'build', '--locked', '--release', '--target', target, '-p', 'librairii-desktop',
                        '--bin', 'librairii-desktop', '--example', 'scale-native', '--message-format=json'], cwd=ROOT, stdout=out, stderr=err, check=True)
    events = [json.loads(s) for s in (destination / 'compiler-artifacts.jsonl').read_text().splitlines()]
    executables = {}
    for name, kind in [('librairii-desktop', 'bin'), ('scale-native', 'example')]:
        matches = [e for e in events if e.get('reason') == 'compiler-artifact' and e.get('target', {}).get('name') == name
                   and e['target']['kind'] == [kind] and e.get('executable')]
        if len(matches) != 1 or matches[0]['profile']['opt_level'] != '3' or matches[0]['profile']['debug_assertions']:
            raise ValueError('exact production release executable/profile required: ' + name)
        path = destination / name
        shutil.copy2(matches[0]['executable'], path)
        executables[name] = {'path': str(path), 'sha256': scale.digest(path), 'bytes': path.stat().st_size, 'profile': matches[0]['profile']}
    report['executables'] = executables
    (destination / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    helper = None
    if platform.system() == 'Darwin':
        helper = destination / 'resource-macos'
        subprocess.run(['xcrun', 'swiftc', str(ROOT / 'qualification/section-16/resource-macos.swift'), '-o', str(helper)], check=True)
        subprocess.run(['xcrun', 'swiftc', str(ROOT / 'qualification/section-16/input-macos.swift'), '-o', str(destination / 'input-macos')], check=True)
    product = Path(executables['librairii-desktop']['path'])
    observer = Path(executables['scale-native']['path'])
    # Include current generated upstream text and embedded-resource declarations;
    # no test data, observer executable or debug symbols enter the size witness.
    with (destination / 'inventory.log').open('w') as log:
        subprocess.run(['cargo', 'xtask', 'inventory'], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)
    package = destination / 'package'
    package.mkdir()
    if platform.system() == 'Darwin':
        contents = package / 'Librairii.app/Contents'
        (contents / 'MacOS').mkdir(parents=True)
        (contents / 'Resources').mkdir()
        shutil.copy2(product, contents / 'MacOS/librairii-desktop')
        with (contents / 'Info.plist').open('wb') as stream:
            plistlib.dump({'CFBundleExecutable': 'librairii-desktop', 'CFBundleIdentifier': 'dev.librairii.scale',
                          'CFBundleName': 'Librairii', 'CFBundlePackageType': 'APPL', 'CFBundleVersion': '0.0.0',
                          'LSMinimumSystemVersion': '15.0', 'NSHighResolutionCapable': True}, stream)
    else:
        shutil.copy2(product, package / 'librairii-desktop')
    shutil.copytree(ROOT / 'target/quality/notices', package / 'notices')
    shutil.copy2(ROOT / 'target/quality/dependency-inventory.json', package / 'dependency-inventory.json')
    (package / 'scope.txt').write_text('Unsigned internal size witness of current production release executable and upstream notices. UI/resources/translations are compiled into this executable. Installed distribution/signing and release license remain Task18.\n')
    archive = destination / 'current-release.tar.gz'
    with tarfile.open(archive, 'w:gz') as stream:
        stream.add(package, arcname='librairii-scale')
    report['package'] = {'bytes': archive.stat().st_size, 'sha256': scale.digest(archive), 'budget_bytes': 32 * 1024 * 1024,
                         'files': {str(p.relative_to(package)): scale.digest(p) for p in package.rglob('*') if p.is_file()}}
    if archive.stat().st_size > 32 * 1024 * 1024:
        (destination / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
        raise ValueError('current production archive exceeds retained32MiB ceiling')
    failures = []
    try:
        report['resource_runs'] = resources.inspect(product, observer, destination / 'measurements', helper, native)
    except Exception as error:
        failures.append({'lane': 'resources', 'error': str(error)})
    report['display_runs'] = []
    for name, backend in [('gpu-api', 'winit-skia'), ('software', 'winit-skia-software')]:
        for factor in (1, 2):
            try:
                report['display_runs'].append(display.inspect(observer, destination / f'display-{name}-{factor}', backend, native, scale=factor))
            except Exception as error:
                failures.append({'lane': f'display-{name}-{factor}', 'error': str(error)})
    if platform.system() == 'Linux':
        try:
            report['mixed_monitors'] = display.mixed_monitors(observer, destination / 'mixed-monitors', native)
        except Exception as error:
            failures.append({'lane': 'mixed-monitors', 'error': str(error)})
    if (scale.output(['git', 'rev-parse', 'HEAD']) != source or scale.output(['git', 'status', '--porcelain', '--untracked-files=normal'])
            or scale.digest(ROOT / 'Cargo.lock') != report['cargo_lock_sha256']
            or any(scale.digest(Path(e['path'])) != e['sha256'] for e in executables.values())):
        raise ValueError('source/lock/executable changed during resource measurement')
    report['failures'] = failures
    report['complete'] = not failures
    report['files'] = {str(p.relative_to(destination)): scale.digest(p) for p in destination.rglob('*') if p.is_file() and p.name != 'manifest.json'}
    (destination / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    if failures:
        raise ValueError('resource/display qualification failed: ' + json.dumps(failures))
    print(json.dumps({'source': source, 'current_release_resources_and_display': 'passed', 'archive_bytes': archive.stat().st_size}))


if __name__ == '__main__':
    main()
