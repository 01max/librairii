"""Read-only frozen-source/derivative policy; no Go execution or parity claim."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import sys

INDEX_SHA = '10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99'
BUNDLE = 'go-1.26.5-xtext-0.43.0-unicode15-cldr32'
DERIVATIVES = 'crates/librairii-storage/src/migrations/compatibility'
EXCLUDED = ['regionTypes.ccTLD-bit1-value2', 'internal/language.Region.TLD', 'language.Region.TLD']
NOTICE_REQUIREMENTS = {
    'normalization': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt', 'LICENSE-UNICODE-V3.txt'],
    'language-default-without-dns': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt',
        'LICENSE-CLDR32-UNICODE.txt', 'LICENSE-CLDR32-APACHE.txt', 'LICENSE-CC0.txt'],
    'product-boundary': [],
}


NOTICE_ORIGINS = {
    'LICENSE-GO-BSD.txt': 'go-xtext-LICENSE.txt',
    'PATENTS-GO.txt': 'go-xtext-PATENTS.txt',
    'LICENSE-UNICODE-V3.txt': 'current/unicode-license.txt',
    'LICENSE-CLDR32-UNICODE.txt': 'cldr32-unicode-license.txt',
    'LICENSE-CLDR32-APACHE.txt': 'cldr32-apache-license.txt',
    'LICENSE-CC0.txt': 'cc0-1.0-legalcode.txt',
    'PROVENANCE.md': None,
}


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
            or any(p in ('', '.', '..') for p in value.split('/'))):
        raise ValueError('noncanonical source-policy path')
    return value


def safe(root, value):
    value = relative(value)
    current = root
    for part in value.split('/'):
        current = current / part
        if current.is_symlink() or getattr(current, 'is_junction', lambda: False)():
            raise ValueError('aliased source-policy path')
    return current


def pinned(root, record):
    path = safe(root, record['path'])
    if not path.is_file():
        raise ValueError('missing source-policy file')
    data = path.read_bytes()
    if len(data) != record['bytes'] or digest(data) != record['sha256']:
        raise ValueError('changed source-policy file')
    return data


def selection(path):
    if path.startswith(('golang.org/x/text@v0.43.0/internal/language/',
                        'golang.org/x/text@v0.43.0/internal/tag/',
                        'golang.org/x/text@v0.43.0/language/')):
        return ['language-default-without-dns']
    if path.endswith(('internal/searchtext/searchtext.go', 'internal/library/story_query.go')):
        return ['product-boundary']
    return ['normalization']


def check(root):
    registry = decode(safe(root, '.config/upstream-sources.json').read_bytes())
    if (set(registry) != {'version', 'product_license', 'bundles'} or registry['version'] != 1
            or registry['product_license'] is not None or len(registry['bundles']) != 1):
        raise ValueError('source registry must not assign a product license or widen scope')
    bundle = registry['bundles'][0]
    if (bundle['id'] != BUNDLE or bundle['scope'] != 'private-storage-4.4'
            or bundle['source_index'] != '.config/upstream-sources/' + BUNDLE + '/source-index.json'
            or bundle['notices_root'] != '.config/upstream-notices/' + BUNDLE
            or bundle['source_index_sha256'] != INDEX_SHA
            or bundle['source_identities'] != 49 or bundle['package_files'] != 43
            or bundle['derivative_root'] != DERIVATIVES
            or bundle['derivative_manifest'] != 'source-manifest.json'
            or bundle['excluded'] != EXCLUDED
            or bundle['selection_notices'] != NOTICE_REQUIREMENTS):
        raise ValueError('unreviewed source selection, scope or DNS policy')
    index_path = safe(root, bundle['source_index'])
    data = index_path.read_bytes()
    if digest(data) != INDEX_SHA:
        raise ValueError('changed authenticated source index')
    if {p.relative_to(index_path.parent).as_posix() for p in index_path.parent.rglob('*')} != {'source-index.json'}:
        raise ValueError('extra files beside authenticated source index')
    index = decode(data)
    sources = [f for package in index['packages'] for f in package['files']]
    if len(sources) != 43:
        raise ValueError('changed package-source set')
    sources += index['supplementalSourceFiles']
    by_path = {f['path']: f for f in sources}
    if len(sources) != 49 or len(by_path) != 49:
        raise ValueError('changed or duplicate source identities')
    if bundle['source_selections'] != {p: selection(p) for p in by_path}:
        raise ValueError('unreviewed source subset')
    for source in sources:
        relative(source['path'])
        header = bytes.fromhex(source['headerFirst12LinesHex'])
        if digest(header) != source['first12LinesSha256']:
            raise ValueError('changed original source-header identity')
    notice_root = safe(root, bundle['notices_root'])
    notice_records = bundle['notices']
    names = {r['path'] for r in notice_records}
    expected_names = set().union(*NOTICE_REQUIREMENTS.values()) | {'PROVENANCE.md'}
    if names != expected_names or len(names) != len(notice_records):
        raise ValueError('missing or duplicate notice inventory')
    actual_names = set()
    for path in notice_root.rglob('*'):
        name = path.relative_to(notice_root).as_posix()
        safe(notice_root, name)
        if path.is_file():
            actual_names.add(name)
    if names != actual_names:
        raise ValueError('extra or missing notice files')
    for record in notice_records:
        if (set(record) != {'path', 'bytes', 'sha256', 'upstream_path'}
                or record['upstream_path'] != NOTICE_ORIGINS[record['path']]):
            raise ValueError('unreviewed upstream notice origin')
        pinned(notice_root, record)
        if record['upstream_path'] is not None:
            original = next(r for r in index['noticePayloads'] if r['path'] == record['upstream_path'])
            if (record['sha256'], record['bytes']) != (original['sha256'], original['bytes']):
                raise ValueError('notice differs from authenticated handoff')
    derivative_root = safe(root, DERIVATIVES)
    records = []
    if derivative_root.exists():
        manifest_path = safe(derivative_root, 'source-manifest.json')
        manifest = decode(manifest_path.read_bytes())
        if set(manifest) != {'version', 'bundle', 'files'} or manifest['version'] != 1 or manifest['bundle'] != BUNDLE:
            raise ValueError('invalid private derivative manifest')
        records = manifest['files']
        actual = set()
        for path in derivative_root.rglob('*'):
            name = path.relative_to(derivative_root).as_posix()
            safe(derivative_root, name)
            if path.is_file() and name != 'source-manifest.json':
                actual.add(name)
        if not records or len({r['path'] for r in records}) != len(records) or {r['path'] for r in records} != actual:
            raise ValueError('unrecorded or missing private derivative')
        for record in records:
            if set(record) != {'path', 'bytes', 'sha256', 'kind', 'recipe', 'sources', 'notices'}:
                raise ValueError('invalid derivative record')
            content = pinned(derivative_root, record)
            if not isinstance(record['recipe'], str) or not record['recipe'].strip():
                raise ValueError('missing original-to-derived recipe')
            kind = record['kind']
            if kind == 'authored':
                if record['sources'] or record['notices']:
                    raise ValueError('authored glue cannot claim upstream provenance')
            elif kind in ('translated', 'generated') and record['sources']:
                required = set()
                seen = set()
                for use in record['sources']:
                    if set(use) != {'path', 'sha256', 'header_sha256', 'selection'} or use['path'] in seen:
                        raise ValueError('duplicate or invalid derivative source')
                    seen.add(use['path'])
                    original = by_path.get(use['path'])
                    if (original is None or use['sha256'] != original['sha256']
                            or use['header_sha256'] != original['first12LinesSha256']
                            or use['selection'] not in bundle['source_selections'][use['path']]):
                        raise ValueError('unapproved original identity or source subset')
                    required.update(NOTICE_REQUIREMENTS[use['selection']])
                    header = bytes.fromhex(original['headerFirst12LinesHex'])
                    for line in header.splitlines():
                        if line.startswith(b'// Copyright') and line not in content:
                            raise ValueError('dropped original source copyright header')
                if set(record['notices']) != required or len(set(record['notices'])) != len(record['notices']):
                    raise ValueError('incomplete derivative notice attribution')
            else:
                raise ValueError('invalid derivative kind or missing original sources')
    return {
        'version': 1, 'bundle': BUNDLE, 'product_license': None,
        'source_index_sha256': INDEX_SHA, 'source_identities': sources,
        'notices': notice_records, 'private_derivatives': records,
        'state': 'private-provenance-recorded' if records else 'awaiting-private-implementation',
        'excluded': EXCLUDED,
        'qualification': 'source inventory only; no algorithm, parity, effect or platform qualification',
    }


def inventory(root, report):
    source = safe(root, '.config/upstream-notices/' + BUNDLE)
    destination = safe(root, 'target/quality/notices/' + BUNDLE)
    destination.mkdir(parents=True, exist_ok=True)
    for record in report['notices']:
        output = safe(destination, record['path'])
        shutil.copyfile(safe(source, record['path']), output)
        pinned(destination, record)
    summary = safe(root, 'target/quality/notices/NOTICES-frozen-sources.txt')
    summary.write_text('Private frozen-source notices. Product license remains undecided.\n'
        + BUNDLE + '\nState: ' + report['state'] + '\n'
        + report['qualification'] + '\nFiles: ' + BUNDLE + '/\n', encoding='utf-8')


def main():
    if sys.argv[1:] not in ([], ['--inventory']):
        raise ValueError('usage: check_sources.py [--inventory]')
    root = Path(__file__).resolve().parent.parent
    report = check(root)
    if sys.argv[1:] == ['--inventory']:
        inventory(root, report)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
