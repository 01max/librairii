"""Read exact migration bytes only; never rewrite SQL, open DBs or execute Go."""
import hashlib
import importlib.util
import json
from pathlib import Path

NAMESPACES = {
    'storage': 'crates/librairii-storage/migrations',
    'go': 'internal/database/migrations',
    'corpus': 'preservation/corpus/migrations',
}


def read(root, relative):
    path = root
    for part in relative.split('/'):
        path = path / part
        if path.is_symlink() or getattr(path, 'is_junction', lambda: False)():
            raise ValueError('aliased migration input: ' + relative)
    return path.read_bytes()


def check(root):
    spec = importlib.util.spec_from_file_location('admitted_migration_pins', root / 'xtask/check_preservation.py')
    pins = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(pins)
    baseline = json.loads(read(root, '.config/rust-preservation-baseline.json'))
    pins.verify_pins(root, baseline)
    manifest = json.loads(read(root, 'preservation/corpus/manifest.json'))
    entries = sorted((r for r in manifest['files'] if r['path'].startswith('migrations/')), key=lambda r: r['path'])
    if len(entries) != 18 or [r['path'].split('/')[-1][:3] for r in entries] != [f'{v:03}' for v in range(1, 19)]:
        raise ValueError('wrong admitted migration inventory')
    names = {r['path'].split('/')[-1] for r in entries}
    for directory in NAMESPACES.values():
        if {p.name for p in (root / directory).glob('*.sql')} != names:
            raise ValueError('changed migration path set: ' + directory)
    results = []
    for entry in entries:
        name = entry['path'].split('/')[-1]
        hashes = {}
        for kind, directory in NAMESPACES.items():
            relative = directory + '/' + name
            data = read(root, relative)
            actual = hashlib.sha256(data).hexdigest()
            if len(data) != entry['bytes'] or actual != entry['sha256']:
                raise ValueError('migration bytes changed: ' + relative)
            hashes[kind + '_sha256'] = actual
        results.append({'filename': name, 'bytes': entry['bytes'], 'admitted_sha256': entry['sha256'], **hashes})
    return {'oracle_commit': manifest['oracle_commit'], 'manifest_sha256': baseline['manifest_sha256'],
            'migration_count': len(results), 'mode': 'read-only exact bytes; no newline normalization', 'migrations': results}


if __name__ == '__main__':
    print(json.dumps(check(Path(__file__).resolve().parent.parent), indent=2))
