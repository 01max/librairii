"""Verify the closed source tree and explicit changes of our pinned archive patch."""
import hashlib
import json
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE_SHA = '110c472f8f3db3fb0f3a41d9e288e36bb754f3dc0475a6c549d7754cdca62bed'
ORIGINAL_INDEX_SHA = 'b83bde8ddaa880404c3d0dbabd1fd67767deecc3bfe4e477b67ccd56888a27c9'
SOURCE = 'https://static.crates.io/crates/sevenz-turbo/sevenz-turbo-0.29.2.crate'
REVISION = '376143a85ddce67698df114eead4c702fef15e95'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f'duplicate archive patch field: {key}')
        result[key] = value
    return result


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def safe(root, relative):
    path = root
    for part in Path(relative).parts:
        require(part not in ('..', '.'), 'archive patch traversal')
        path = path / part
        require(not path.is_symlink(), f'archive patch symlink: {relative}')
    require(not path.is_file() or path.stat().st_nlink == 1,
            f'archive patch hard link: {relative}')
    return path


def check(root=ROOT):
    root = Path(root)
    raw = safe(root, '.config/sevenz-turbo-patch.json').read_bytes()
    pin = json.loads(raw, object_pairs_hook=unique)
    require(pin['name'] == 'sevenz-turbo' and pin['version'] == '0.29.2', 'archive patch identity')
    require(pin['archive_sha256'] == ARCHIVE_SHA and pin['source'] == SOURCE
            and pin['git_revision'] == REVISION, 'archive patch upstream identity')
    require(hashlib.sha256(encoded(pin['files'])).hexdigest() == ORIGINAL_INDEX_SHA,
            'original published source index changed')
    require(set(pin['patched_files']) == {'src/reader.rs', 'src/codec/brotli.rs'},
            'unreviewed modified-file set')
    tree = safe(root, 'third_party/sevenz-turbo')
    actual = {}
    for path in tree.rglob('*'):
        relative = path.relative_to(root).as_posix()
        path = safe(root, relative)
        require(path.is_file() or path.is_dir(), f'non-regular archive source: {relative}')
        if path.is_file():
            data = path.read_bytes()
            actual[path.relative_to(tree).as_posix()] = {
                'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
    expected = dict(pin['files'])
    expected.update(pin['patched_files'])
    require(actual == expected, 'vendored source added, removed or changed outside the pinned patch')
    for path, relative in [('Cargo.toml', 'third_party/sevenz-turbo'),
                           ('fuzz/Cargo.toml', '../third_party/sevenz-turbo')]:
        doc = tomllib.loads(safe(root, path).read_text())
        require(doc['patch']['crates-io']['sevenz-turbo'] == {'path': relative},
                'archive source override changed')
    return {'name': pin['name'], 'version': pin['version'], 'upstream': pin['source'],
            'upstream_checksum': pin['archive_sha256'], 'upstream_revision': pin['git_revision'],
            'tree_sha256': hashlib.sha256(encoded(actual)).hexdigest(),
            'patch_manifest_sha256': hashlib.sha256(raw).hexdigest(),
            'patched_files': pin['patched_files'], 'notice': 'third_party/sevenz-turbo/LICENSE'}


if __name__ == '__main__':
    try:
        print(json.dumps(check(), indent=2))
    except (OSError, KeyError, TypeError, ValueError) as error:
        sys.exit('Archive patch provenance failed: ' + str(error))
