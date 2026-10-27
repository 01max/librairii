"""Seal the promoted native patch and retain its native dependency notices."""
import hashlib
import json
from pathlib import Path
import shutil
import tomllib
ROOT = Path(__file__).resolve().parents[1]
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def inventory():
    pin = json.loads((ROOT / '.config/native-toolkit.json').read_text())
    cargo = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    for name, expected in [('slint', '1.18.1'), ('winit', '0.30.13'), ('skia-bindings', '0.153.3')]:
        if {p['version'] for p in lock['package'] if p['name'] == name} != {expected}:
            raise ValueError('native graph changed; qualification must be reopened: ' + name)
    dep = cargo['workspace']['dependencies']['slint']
    if dep['version'] != '=1.18.1' or dep['default-features'] or dep['features'] != pin['features']:
        raise ValueError('unqualified native renderer/features')
    patch = pin['patch']
    directory = ROOT / patch['path']
    if cargo['patch']['crates-io'][patch['name']]['path'] != patch['path']:
        raise ValueError('native patch resolution changed')
    actual = {}
    for path in sorted(directory.rglob('*')):
        if path.is_symlink():
            raise ValueError('native patch contains a symlink')
        if path.is_file():
            actual[str(path.relative_to(directory))] = sha(path)
    if actual != patch['files']:
        raise ValueError('qualified AccessKit source/notice tree changed')
    origin = json.loads((directory / 'UPSTREAM.json').read_text())
    if (origin['version'] != '0.19.1' or origin['backport_commit'] != '6ee0558b6315b3ef1594db24ce45a030ecac7cb5'):
        raise ValueError('qualified AccessKit provenance changed')
    source = ROOT / 'qualification/section-2/notices/native-skia'
    if sha(source / 'inventory.json') != pin['native_notice_inventory_sha256']:
        raise ValueError('native Skia notice inventory changed')
    notices = json.loads((source / 'inventory.json').read_text())
    if notices['skia_bindings_version'] != '0.153.3' or notices['skia_source_ref'] != pin['skia_source_ref']:
        raise ValueError('native notices do not match selected Skia')
    if {p.name for p in source.iterdir()} != set(notices['files']) | {'inventory.json'}:
        raise ValueError('native notice inventory has extra or missing files')
    for name, digest in notices['files'].items():
        path = source / name
        if path.name != name or path.is_symlink() or sha(path) != digest:
            raise ValueError('native notice payload changed')
    destination = ROOT / 'target/quality/notices/native-skia'
    destination.mkdir(parents=True, exist_ok=True)
    for path in source.iterdir():
        shutil.copy2(path, destination / path.name)
    value = {'name': patch['name'], 'version': patch['version'], 'directory': patch['path'], 'upstream': patch['upstream'], 'tree_sha256': hashlib.sha256(json.dumps(actual, sort_keys=True).encode()).hexdigest(), 'upstream_provenance': origin, 'skia_source_ref': pin['skia_source_ref'], 'native_notices': {'inventory_sha256': pin['native_notice_inventory_sha256'], 'scope': notices['scope']}}
    (ROOT / 'target/quality/native-toolkit-inventory.json').write_text(json.dumps(value, indent=2) + '\n')
    return value
if __name__ == '__main__':
    print(json.dumps(inventory(), sort_keys=True))
