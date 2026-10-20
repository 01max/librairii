"""Committed private JSONv1 provenance, without Go/network or a parity claim."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import sys

BUNDLE = 'go-1.26.5-json-v1-studio'
INDEX_SHA = 'd63835ff65b581d3e0be5855189f59ac2fa4924a65904956de1f65e79309a4f4'
SCHEMA_SHA = 'a0d54e23c9150f99f849383133ca32068465c566f9102b1cdf4de98b9367d1f7'
METADATA = '.config/upstream-sources/' + BUNDLE
NOTICES = '.config/upstream-notices/' + BUNDLE
DERIVATIVES = 'crates/librairii-core/src/import/studio_json'
KINDS = {DERIVATIVES + '/' + name: kind for name, kind in [
    ('mod.rs', 'translated'), ('scanner.rs', 'translated'),
    ('strings.rs', 'translated'), ('tests.rs', 'authored')]}
GO = 'Go1.26.5/src/'
SOURCE_SETS = {
    DERIVATIVES + '/mod.rs': {GO + 'encoding/json/decode.go', GO + 'encoding/json/stream.go',
        'oracle73a93303/internal/inspection/studio_zip.go',
        'oracle73a93303/internal/inspection/zip_inspector.go'},
    DERIVATIVES + '/scanner.rs': {GO + 'encoding/json/scanner.go'},
    DERIVATIVES + '/strings.rs': {GO + name for name in [
        'encoding/json/decode.go', 'encoding/json/fold.go', 'unicode/letter.go',
        'unicode/tables.go', 'unicode/utf8/utf8.go', 'unicode/utf16/utf16.go']},
    DERIVATIVES + '/tests.rs': set(),
}
BASE_NOTICES = ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt']
REQUIREMENTS = {selection: list(BASE_NOTICES) for selection in [
    'json-v1-scanner', 'json-v1-fixed-reducer', 'json-v1-field-fold',
    'json-v1-decoder-order', 'unicode15-simple-fold', 'go-json-utf8', 'go-json-utf16']}
for selection in ['json-v1-field-fold', 'unicode15-simple-fold']:
    REQUIREMENTS[selection].append('LICENSE-UNICODE-V3.txt')
REQUIREMENTS['studio-product-boundary'] = []


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate source-policy JSON field')
        result[key] = value
    return result


def decode(data):
    return json.loads(data.decode('utf-8'), object_pairs_hook=unique_object)


def relative(value):
    if (not isinstance(value, str) or not value or '\\' in value or ':' in value
            or PurePosixPath(value).is_absolute()
            or any(part in ('', '.', '..') for part in value.split('/'))
            or any(ord(char) < 32 for char in value)):
        raise ValueError('noncanonical source-policy path')
    return value


def safe(root, value):
    current = root
    for part in relative(value).split('/'):
        current = current / part
        if current.is_symlink() or getattr(current, 'is_junction', lambda: False)():
            raise ValueError('aliased source-policy path')
    return current


def pinned(root, record):
    path = safe(root, record['path'])
    if not path.is_file():
        raise ValueError('missing source-policy file')
    data = path.read_bytes()
    if (type(record['bytes']) is not int or len(data) != record['bytes']
            or digest(data) != record['sha256']):
        raise ValueError('changed source-policy file')
    return data


def exact_children(root, folder, expected):
    directory = safe(root, folder)
    actual = set()
    for child in directory.iterdir():
        path = folder + '/' + child.name
        if not safe(root, path).is_file():
            raise ValueError('nonregular or extra source-policy entry')
        actual.add(path)
    if actual != expected:
        raise ValueError('extra or missing source-policy file')


def check(root):
    registry = decode(safe(root, '.config/upstream-sources.json').read_bytes())
    if (set(registry) != {'version', 'product_license', 'bundles'}
            or type(registry['version']) is not int or registry['version'] != 1
            or registry['product_license'] is not None
            or len(registry['bundles']) != 1):
        raise ValueError('unreviewed source registry or product license')
    bundle = registry['bundles'][0]
    expected = {
        'id': BUNDLE, 'scope': 'private-core-import-studio-json-v1',
        'source_index': METADATA + '/source-index.json', 'source_index_sha256': INDEX_SHA,
        'source_schema': METADATA + '/source-manifest.schema.json', 'source_schema_sha256': SCHEMA_SHA,
        'source_identities': 18, 'notices_root': NOTICES, 'derivative_root': DERIVATIVES,
        'derivative_manifest': 'source-manifest.json', 'selection_notices': REQUIREMENTS,
        'required_derivatives': KINDS,
        'qualification': 'source inventory only; no behavior, resource, effect or platform qualification',
    }
    if (set(bundle) != set(expected) | {'notices', 'source_selections'}
            or any(bundle[key] != value for key, value in expected.items())):
        raise ValueError('unreviewed source scope, identities or schema')
    exact_children(root, METADATA, {bundle['source_index'], bundle['source_schema']})
    data = safe(root, bundle['source_index']).read_bytes()
    if digest(data) != INDEX_SHA or digest(safe(root, bundle['source_schema']).read_bytes()) != SCHEMA_SHA:
        raise ValueError('changed authenticated source index or manifest schema')
    index = decode(data)
    originals = {source['path']: source for source in index['sources']}
    if (len(originals) != 18 or bundle['source_selections'] != {
            path: source['selection'] for path, source in originals.items()}):
        raise ValueError('changed source selection')
    for source in originals.values():
        relative(source['path'])
        if digest(bytes.fromhex(source['header_first_12_lines_hex'])) != source['header_sha256']:
            raise ValueError('changed original source-header identity')
    notices = bundle['notices']
    by_notice = {notice['path']: notice for notice in notices}
    original_notices = {notice['path']: notice for notice in index['notices']}
    if len(by_notice) != len(notices) or set(by_notice) != set(original_notices) | {'PROVENANCE.md'}:
        raise ValueError('missing or duplicate notice inventory')
    exact_children(root, NOTICES, {NOTICES + '/' + name for name in by_notice})
    for name, notice in by_notice.items():
        if set(notice) != {'path', 'bytes', 'sha256', 'origin'}:
            raise ValueError('invalid notice record')
        # A license cannot become a self-repinnable authored notice by dropping
        # its authenticated origin. Only our policy prose has no upstream origin.
        if (name == 'PROVENANCE.md' and notice['origin'] is not None
                or name != 'PROVENANCE.md' and notice != original_notices[name]):
            raise ValueError('changed upstream notice identity or origin')
        pinned(safe(root, NOTICES), notice)
    manifest_path = DERIVATIVES + '/source-manifest.json'
    manifest = decode(safe(root, manifest_path).read_bytes())
    if (set(manifest) != {'version', 'bundle', 'files'}
            or type(manifest['version']) is not int or manifest['version'] != 1
            or manifest['bundle'] != BUNDLE):
        raise ValueError('invalid private source manifest')
    records = manifest['files']
    names = {record['path'] for record in records}
    if len(names) != len(records) or names != set(KINDS):
        raise ValueError('unreviewed or duplicate derivative inventory')
    exact_children(root, DERIVATIVES, names | {manifest_path})
    for record in records:
        if (set(record) != {'path', 'bytes', 'sha256', 'kind', 'recipe', 'sources', 'notices'}
                or record['kind'] != KINDS[record['path']]
                or not isinstance(record['recipe'], str) or not record['recipe'].strip()):
            raise ValueError('invalid derivative kind, record or recipe')
        content = pinned(root, record)
        sources = record['sources']
        if {use['path'] for use in sources} != SOURCE_SETS[record['path']] or len({use['path'] for use in sources}) != len(sources):
            raise ValueError('dropped, duplicate or unreviewed original source')
        required = set()
        for use in sources:
            if set(use) != {'path', 'sha256', 'header_sha256', 'selection'}:
                raise ValueError('invalid original-to-derived record')
            source = originals.get(use['path'])
            if (source is None or source['permission'] not in (
                    'bounded-translation-reference', 'product-contract-reference')
                    or any(use[key] != source[key] for key in ['sha256', 'header_sha256', 'selection'])):
                raise ValueError('unapproved original identity, permission or selection')
            required.update(REQUIREMENTS[use['selection']])
            for line in bytes.fromhex(source['header_first_12_lines_hex']).splitlines():
                if line.startswith(b'// Copyright') and line not in content:
                    raise ValueError('dropped original source copyright header')
        if set(record['notices']) != required or len(record['notices']) != len(required):
            raise ValueError('incomplete derivative notice union')
    return {'version': 1, 'bundle': BUNDLE, 'product_license': None,
        'source_index_sha256': INDEX_SHA, 'source_identities': index['sources'],
        'notices': notices, 'private_derivatives': records,
        'qualification': bundle['qualification']}


def inventory(root, report):
    folder = 'target/quality/notices/' + BUNDLE
    destination = safe(root, folder)
    destination.mkdir(parents=True, exist_ok=True)
    for notice in report['notices']:
        target = safe(root, folder + '/' + notice['path'])
        shutil.copyfile(safe(root, NOTICES + '/' + notice['path']), target)
        pinned(destination, notice)
    safe(root, 'target/quality/notices/NOTICES-import-frozen-sources.txt').write_text(
        'Private fixed-shape studio JSONv1 upstream notices. Product license remains undecided.\n'
        + BUNDLE + '\n' + report['qualification'] + '\nFiles: ' + BUNDLE + '/\n', encoding='utf-8')


def main():
    if sys.argv[1:] not in ([], ['--inventory']):
        raise ValueError('usage: check_import_sources.py [--inventory]')
    root = Path(__file__).resolve().parent.parent
    report = check(root)
    if sys.argv[1:] == ['--inventory']:
        inventory(root, report)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
