#!/usr/bin/env python3
"""Package the synthetic spike only. No signing, installation, or product release."""
import argparse
import hashlib
import json
import pathlib
import platform
import plistlib
import shutil
import subprocess
import tarfile

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
TARGETS = {
    'aarch64-apple-darwin': ('Darwin', 'arm64'),
    'x86_64-apple-darwin': ('Darwin', 'x86_64'),
    'x86_64-unknown-linux-gnu': ('Linux', 'x86_64'),
}

def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            digest.update(block)
    return digest.hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=pathlib.Path, required=True)
    parser.add_argument('--target', choices=TARGETS, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    if (platform.system(), platform.machine()) != TARGETS[args.target]:
        parser.error('packages must be assembled on the matching native target')
    binary = args.binary.resolve(strict=True)
    architecture = subprocess.check_output(['file', str(binary)], text=True)
    expected = 'arm64' if args.target.startswith('aarch64') else ('x86_64' if platform.system() == 'Darwin' else 'x86-64')
    if expected not in architecture:
        parser.error('binary architecture does not match the package target')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=REPO))
    if platform.system() == 'Darwin':
        app = output / 'LibrairiiQualification.app' / 'Contents'
        (app / 'MacOS').mkdir(parents=True)
        executable = app / 'MacOS' / 'qualification-desktop'
        with (app / 'Info.plist').open('wb') as stream:
            plistlib.dump({'CFBundleExecutable': 'qualification-desktop',
                'CFBundleIdentifier': 'dev.librairii.qualification',
                'CFBundleName': 'Librairii Qualification', 'CFBundlePackageType': 'APPL',
                'CFBundleVersion': '0.0.0', 'LSMinimumSystemVersion': '15.0',
                'NSHighResolutionCapable': True}, stream)
    else:
        executable = output / 'qualification-desktop'
        (output / 'librairii-qualification.desktop').write_text(
            '[Desktop Entry]\nType=Application\nName=Librairii Qualification\n'
            'Exec=qualification-desktop\nTerminal=false\nCategories=Utility;\n')
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    for name in ('operator-checks.md', 'observe.py', 'current-pins.json', 'continuation.md'):
        shutil.copy2(HERE / name, output / name)
    notices = output / 'notices'
    notices.mkdir()
    shutil.copy2(HERE / 'notices' / 'Slint-Royalty-free-2.0.md', notices)
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked',
        '--format-version', '1', '--features', 'gui', '--filter-platform', args.target], cwd=HERE))
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    inventory = []
    for package in metadata['packages']:
        if package['id'] not in resolved:
            continue
        crate = pathlib.Path(package['manifest_path']).parent
        files = [p for p in crate.iterdir() if p.is_file() and p.name.lower().startswith(('license', 'copying', 'notice'))]
        if package.get('license_file'):
            files.append(crate / package['license_file'])
        entries = []
        for item in sorted(set(files)):
            if not item.is_file():
                raise RuntimeError('declared license file missing: ' + package['name'])
            dest = notices / (package['name'] + '-' + package['version'])
            dest.mkdir(exist_ok=True)
            shutil.copy2(item, dest / item.name)
            entries.append({'file': str((dest / item.name).relative_to(output)), 'sha256': sha(item)})
        inventory.append({'name': package['name'], 'version': package['version'],
            'license': package.get('license'), 'source': package.get('source'), 'notices': entries})
    # rust-skia distributes the native library's BSD notice in the build output.
    native = list(binary.parent.glob('build/skia-bindings-*/out/skia/LICENSE_SKIA'))
    if not native or len({sha(path) for path in native}) != 1:
        raise RuntimeError('missing or conflicting native Skia licenses beside the build output')
    shutil.copy2(native[0], notices / 'LICENSE_SKIA')
    (output / 'dependencies.json').write_text(json.dumps(inventory, indent=2) + '\n')
    if platform.system() == 'Darwin':
        resources = app / 'Resources'
        resources.mkdir()
        shutil.copytree(notices, resources / 'notices')
        shutil.copy2(output / 'dependencies.json', resources)
    manifest = {'schema': 1, 'kind': 'unsigned-synthetic-qualification-only',
        'source': source, 'source_dirty': dirty, 'target': args.target,
        'features': ['gui'], 'executable': str(executable.relative_to(output)),
        'binary_sha256': sha(executable), 'lock_sha256': sha(HERE / 'Cargo.lock'),
        'architecture': architecture.strip(), 'toolkit_selected': False,
        'qualification': 'pending operator observations and independent final gate',
        'notice_scope': 'registry top-level/declared notices plus native Skia BSD; full production native notice audit remains open'}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    # The manifest describes every distributed file. Its own digest is external.
    manifest['files'] = {str(p.relative_to(output)): sha(p) for p in sorted(output.rglob('*')) if p.is_file() and p.name != 'manifest.json'}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    archive = output.parent / (output.name + '.tar.gz')
    if archive.exists():
        raise FileExistsError(archive)
    with tarfile.open(archive, 'x:gz') as stream:
        stream.add(output, arcname=output.name)
    archive.with_suffix(archive.suffix + '.sha256').write_text(sha(archive) + '  ' + archive.name + '\n')
    if platform.system() == 'Darwin':
        dmg = output.parent / (output.name + '.dmg')
        if dmg.exists():
            raise FileExistsError(dmg)
        subprocess.run(['hdiutil', 'create', '-volname', 'Librairii Qualification',
                        '-srcfolder', str(output), '-format', 'UDZO', str(dmg)], check=True)
        dmg.with_suffix('.dmg.sha256').write_text(sha(dmg) + '  ' + dmg.name + '\n')
    print(archive)

if __name__ == '__main__':
    main()
